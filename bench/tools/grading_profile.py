#!/usr/bin/env python3
"""Profile archived grading attempts: billed usage by category, tool turns, retries and cost per review and claim.

Usage::

    python3 bench/tools/grading_profile.py --archives bench/regrading/<queue> --completion <completion-audit.json> \\
        [--markdown] [--root REPOSITORY]

``--archives`` holds ``<run>/<target>/attempt-<n>/evidence.json`` beside the ``evidence.tar.gz`` it pins, as
``regrade.py`` archives them: ``key.json`` (``reviews`` with ``items``), ``work/dispatch.json`` and the session
transcript under ``work/home/.claude/projects/``. ``--completion`` is the completion audit whose ``batches`` pin the
accepted attempt's ``evidence`` and its ``mapping`` by repository-relative path and SHA-256. Every other archived
attempt is a failed or replaced one. Archives are read in place and never extracted into the repository. Pinned
paths and ``bench/rates.json`` resolve against ``--root``, by default this repository.

Usage comes from ``transcript_usage.read_transcript``, which counts the lines sharing a ``requestId`` as one billed
request, priced at the latest ``bench/rates.json`` entry for the dispatched model. Tokens are the provider's recorded
counts; file and tool-result sizes are never converted into tokens.

The exploration estimate reads each session's requests in order. A request's context is its fresh input plus cache
writes plus cache reads; the growth between consecutive requests is what the earlier turn added: its output and its
tool results. That growth is attributed to the turn's tool calls and priced as one cache write at the
session's own average write price plus a cache read in every later request. The first request's context is priced
as recorded and then as a cache read in every later request. ``unattributed_cache_cost_usd`` is the recorded input
and cache cost less everything the estimate attributes.

A tool call is ``verdicts`` when it names ``verdicts.json`` or is a verdict tool, including the scripts graders write
to generate verdicts; otherwise ``source`` when it names the pinned clone or its cache, ``source+inputs`` when it
also names a prepared grading file, ``inputs`` when it names only prepared files, and ``unlabelled`` when it names
none of them (scratch probes, and commands run after ``cd clone``). A turn with several calls is ``verdicts`` if any
call is, and otherwise ``source+inputs`` when its labelled calls differ. Source inspection therefore lies between the
``source`` row and the sum of ``source``, ``source+inputs`` and ``unlabelled``.

Output is JSON, or with ``--markdown`` the same figures as a report. Exit codes: 0 printed; 1 a pinned input changed
or a cross-check failed, one line per problem on stdout; 2 an input cannot be read, named on stderr.
"""

from __future__ import annotations

import argparse
from collections import Counter
from datetime import datetime
import hashlib
import json
from pathlib import Path
import re
import statistics
import sys
import tarfile
import tempfile

TOOLS = Path(__file__).resolve().parent
ROOT = TOOLS.parents[1]
sys.path.insert(0, str(TOOLS))
import transcript_usage  # noqa: E402

PREPARED_INPUTS = ("reviews/", "claims.md", "register.json", "rubric.md", "packet.md", "prompt.md", "validator/",
                   "evidence/")
VERDICT_TOOLS = ("mcp__grading__write_verdicts", "mcp__grading__validate")
CATEGORIES = ("source", "source+inputs", "unlabelled", "inputs", "verdicts", "none")
TOKEN_FIELDS = ("input", "cache_write_5m", "cache_write_1h", "cache_write_unknown", "cache_read", "output", "thinking")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def rates_for(model: str, rates_path: Path) -> transcript_usage.Rates:
    entries = [r for r in json.loads(rates_path.read_text(encoding="utf-8"))["rates"] if r["model"] == model]
    if not entries:
        raise ValueError(f"no rates.json entry for {model}")
    rate = sorted(entries, key=lambda r: r["as_of"])[-1]
    return transcript_usage.Rates((rate["input"], rate["output"]), rate["cache_write_5m"] / rate["input"],
                                  rate["cache_write_1h"] / rate["input"], rate["cache_read"] / rate["input"])


def call_category(name: str, data) -> str:
    text = json.dumps(data)
    if name in VERDICT_TOOLS or "verdicts.json" in text:
        return "verdicts"
    source = re.search(r"clone(?!-work)", text) is not None
    inputs = any(marker in text for marker in PREPARED_INPUTS)
    if source:
        return "source+inputs" if inputs else "source"
    return "inputs" if inputs else "unlabelled"


def requests_of(path: Path) -> list:
    """The transcript's billed requests in first-seen order, under ``read_transcript``'s one-request rule."""
    requests, anonymous = {}, 0
    for raw in path.read_text(encoding="utf-8").splitlines():
        try:
            line = json.loads(raw)
        except ValueError:
            continue
        message = line.get("message") if isinstance(line, dict) and line.get("type") == "assistant" else None
        if not isinstance(message, dict) or not isinstance(message.get("usage"), dict):
            continue
        if line.get("isApiErrorMessage") or message.get("model") == "<synthetic>":
            continue
        key = line.get("requestId")
        if not isinstance(key, str) or not key:
            anonymous += 1
            key = ("line", anonymous)
        request = requests.setdefault(key, {"id": key if isinstance(key, str) else None, "input": 0, "cache_write": 0,
                                            "cache_read": 0, "output": 0, "tools": {}})
        usage = message["usage"]
        for field, source in (("input", "input_tokens"), ("cache_write", "cache_creation_input_tokens"),
                              ("cache_read", "cache_read_input_tokens"), ("output", "output_tokens")):
            request[field] = max(request[field], int(usage.get(source) or 0))
        for block in message.get("content") or []:
            if isinstance(block, dict) and block.get("type") == "tool_use":
                request["tools"][block.get("id") or len(request["tools"])] = call_category(
                    block.get("name") or "", block.get("input") or {})
    return list(requests.values())


def turn_category(request: dict) -> str:
    found = set(request["tools"].values())
    if "verdicts" in found:
        return "verdicts"
    if len(found) > 1:
        found.discard("unlabelled")
    return next(iter(found), "none") if len(found) < 2 else "source+inputs"


def exploration(requests: list, rates: transcript_usage.Rates, write_mult: float) -> dict:
    rows = {name: {"turns": 0, "tool_calls": 0, "context_growth_tokens": 0, "carried_cost_usd": 0.0}
            for name in CATEGORIES}
    contexts = [r["input"] + r["cache_write"] + r["cache_read"] for r in requests]
    shrinks = 0
    for index, request in enumerate(requests):
        row = rows[turn_category(request)]
        row["turns"] += 1
        row["tool_calls"] += len(request["tools"])
        if index + 1 == len(requests):
            continue
        growth = contexts[index + 1] - contexts[index]
        if growth < 0:
            shrinks += 1
            continue
        later_requests = len(requests) - index - 2
        row["context_growth_tokens"] += growth
        row["carried_cost_usd"] += growth * rates.price_in * (
            write_mult + later_requests * rates.read_mult) / 1_000_000
    first = requests[0]
    return {"by_turn": rows, "context_shrinks": shrinks,
            "initial_context_carried_cost_usd": rates.price_in * (
                first["input"] + first["cache_write"] * write_mult + first["cache_read"] * rates.read_mult
                + contexts[0] * (len(requests) - 1) * rates.read_mult) / 1_000_000}


def seconds_between(start, end):
    try:
        first, last = (datetime.fromisoformat(value.replace("Z", "+00:00")) for value in (start, end))
    except (AttributeError, ValueError):
        return None
    return (last - first).total_seconds()


def read_attempt(receipt_path: Path, root: Path, problems: list) -> dict:
    receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    archive = root / receipt["archive"]["path"]
    if sha256(archive.read_bytes()) != receipt["archive"]["sha256"]:
        problems.append(f"archive changed: {receipt['archive']['path']}")
    attempt = {"path": str(receipt_path.parent.relative_to(root)), "target": receipt_path.parents[1].name,
               "reviews": 0, "items": 0, "dispatched": False,
               "transcripts": 0, "requests": [], "usage": None, "cost_usd": None, "recorded_cost_usd": None,
               "seconds": None, "reservation_usd": None, "model": None, "exit_code": None, "violations": 0,
               "exploration": None}
    with tarfile.open(archive) as bundle, tempfile.TemporaryDirectory() as scratch:
        members = {member.name: member for member in bundle.getmembers() if member.isfile()}

        def load(name):
            return json.loads(bundle.extractfile(members[name]).read().decode("utf-8")) if name in members else None

        key, record, reservation = load("key.json"), load("work/dispatch.json"), load("reservation.json")
        if key:
            attempt.update(reviews=len(key["reviews"]), items=sum(review["items"] for review in key["reviews"]))
        if reservation:
            attempt["reservation_usd"] = reservation.get("maxBudgetUsd")
        if record:
            attempt.update(dispatched=True, model=record["model"], exit_code=record["exit_code"],
                           violations=len(record["audit_violations"]),
                           recorded_cost_usd=record["usage"]["priced_total_usd"],
                           seconds=seconds_between(record.get("dispatched_at"), record.get("completed_at")))
        transcripts = sorted(name for name in members if name.startswith("work/home/") and name.endswith(".jsonl"))
        attempt["transcripts"] = len(transcripts)
        if len(transcripts) > 1:
            problems.append(f"more than one session transcript: {attempt['path']}")
        for name in transcripts[:1]:
            local = Path(scratch) / "session.jsonl"
            local.write_bytes(bundle.extractfile(members[name]).read())
            try:
                usage = transcript_usage.read_transcript(str(local))
            except SystemExit:
                continue
            requests = requests_of(local)
            totals = ("input", "cache_write", "cache_read", "output")
            if (len(requests), *(sum(r[f] for r in requests) for f in totals)) != (
                    usage.counts["turns"], *(usage.counts[f] for f in totals)):
                problems.append(f"request sequence disagrees with transcript_usage: {attempt['path']}")
            rates = rates_for(attempt["model"], root / "bench/rates.json")
            attempt.update(requests=requests, usage=dict(usage.counts), cost_usd=round(usage.cost(rates), 6),
                           category_cost_usd=category_costs(usage.counts, rates))
            written = usage.counts["cache_write"] * rates.price_in / 1_000_000
            attempt["exploration"] = exploration(
                requests, rates, attempt["category_cost_usd"]["cache_write"] / written if written else rates.write_5m_mult)
            if attempt["recorded_cost_usd"] is not None and abs(attempt["cost_usd"] - attempt["recorded_cost_usd"]) > 1e-6:
                problems.append(f"priced usage differs from dispatch.json: {attempt['path']}")
    return attempt


def category_costs(counts: dict, rates: transcript_usage.Rates) -> dict:
    per_token = rates.price_in / 1_000_000
    return {"fresh_input": counts["input"] * per_token,
            "cache_write": (counts["cache_write_5m"] + counts["cache_write_unknown"]) * per_token * rates.write_5m_mult
            + counts["cache_write_1h"] * per_token * rates.write_1h_mult,
            "cache_read": counts["cache_read"] * per_token * rates.read_mult,
            "visible_output": (counts["output"] - counts["thinking"]) * rates.price_out / 1_000_000,
            "thinking": counts["thinking"] * rates.price_out / 1_000_000}


def failure_reason(attempt: dict) -> str:
    if not attempt["dispatched"]:
        return "not dispatched"
    if not attempt["requests"]:
        return "no billed request"
    if attempt["exit_code"] != 0:
        return "session failed"
    if attempt["violations"]:
        return "access violation"
    return "complete session rejected or replaced"


def distribution(values: list) -> dict:
    if not values:
        return {"count": 0}
    ordered = sorted(values)
    return {"count": len(ordered), "median": statistics.median(ordered),
            "p90": ordered[min(len(ordered) - 1, round(0.9 * (len(ordered) - 1)))], "max": ordered[-1],
            "sum": round(sum(ordered), 6)}


def summarize(attempts: list, claims: int | None) -> dict:
    metered = [a for a in attempts if a["requests"]]
    tokens = {field: sum(a["usage"][field] for a in metered) for field in TOKEN_FIELDS}
    costs = {name: round(sum(a["category_cost_usd"][name] for a in metered), 6)
             for name in ("fresh_input", "cache_write", "cache_read", "visible_output", "thinking")}
    cost = round(sum(a["cost_usd"] for a in metered), 6)
    by_turn = {name: {field: round(sum(a["exploration"]["by_turn"][name][field] for a in metered), 6)
                      for field in ("turns", "tool_calls", "context_growth_tokens", "carried_cost_usd")}
               for name in CATEGORIES}
    reviews, items = sum(a["reviews"] for a in metered), sum(a["items"] for a in metered)
    summary = {
        "attempts": len(attempts), "attempts_with_transcript": sum(a["transcripts"] > 0 for a in attempts),
        "attempts_with_billed_requests": len(metered),
        "transcript_lines": sum(a["usage"]["lines"] for a in metered),
        "billed_requests": sum(a["usage"]["turns"] for a in metered),
        "tool_calls": sum(a["usage"]["tool_calls"] for a in metered),
        "text_only_requests": sum(a["usage"]["text_only_turns"] for a in metered),
        "tokens": tokens, "cost_usd": cost, "cost_by_category_usd": costs,
        "unpriced_attempts": len(attempts) - len(metered),
        "reviews": reviews, "items": items, "claims": claims,
        "cost_per_review_usd": round(cost / reviews, 6) if reviews else None,
        "cost_per_item_usd": round(cost / items, 6) if items else None,
        "cost_per_claim_usd": round(cost / claims, 6) if claims else None,
        "dispatch_seconds": distribution([a["seconds"] for a in metered if a["seconds"] is not None]),
        "reservation_usd": distribution([a["reservation_usd"] for a in attempts if a["reservation_usd"] is not None]),
        "peak_reservation_use": max((round(a["cost_usd"] / a["reservation_usd"], 4) for a in metered
                                     if a["reservation_usd"]), default=None),
        "exploration_estimate": {
            "by_turn": by_turn,
            "initial_context_carried_cost_usd": round(
                sum(a["exploration"]["initial_context_carried_cost_usd"] for a in metered), 6),
            "context_shrinks": sum(a["exploration"]["context_shrinks"] for a in metered)},
    }
    estimate = summary["exploration_estimate"]
    attributed = estimate["initial_context_carried_cost_usd"] + sum(row["carried_cost_usd"] for row in by_turn.values())
    estimate["unattributed_cache_cost_usd"] = round(
        costs["fresh_input"] + costs["cache_write"] + costs["cache_read"] - attributed, 6)
    source = by_turn["source"]["carried_cost_usd"]
    possible = source + by_turn["source+inputs"]["carried_cost_usd"] + by_turn["unlabelled"]["carried_cost_usd"]
    estimate["source_share_of_cost"] = {"low": round(source / cost, 4), "high": round(possible / cost, 4)} if cost else None
    return summary


def profile(archives: Path, completion_path: Path, root: Path) -> tuple:
    problems = []
    completion = json.loads(completion_path.read_text(encoding="utf-8"))
    accepted, claims = {}, 0
    for batch in completion["batches"]:
        accepted[batch["evidence"]["path"]] = batch
        mapping_path = root / batch["mapping"]["path"]
        if sha256(mapping_path.read_bytes()) != batch["mapping"]["sha256"]:
            problems.append(f"mapping changed: {batch['mapping']['path']}")
        mapping = json.loads(mapping_path.read_text(encoding="utf-8"))
        claims += sum(len(item.get("claims", [])) for attempt in mapping["attempts"] for item in attempt["items"])
    groups = {"accepted": [], "failed": []}
    seen_requests, reasons = Counter(), Counter()
    for receipt_path in sorted(archives.glob("*/*/attempt-*/evidence.json")):
        relative = str(receipt_path.relative_to(root))
        attempt = read_attempt(receipt_path, root, problems)
        if relative in accepted:
            if sha256(receipt_path.read_bytes()) != accepted.pop(relative)["evidence"]["sha256"]:
                problems.append(f"accepted evidence receipt changed: {relative}")
            groups["accepted"].append(attempt)
        else:
            reasons[failure_reason(attempt)] += 1
            groups["failed"].append(attempt)
        seen_requests.update(r["id"] for r in attempt["requests"] if r["id"])
    problems.extend(f"accepted evidence is not archived: {path}" for path in sorted(accepted))
    repeated = sum(count > 1 for count in seen_requests.values())
    if repeated:
        problems.append(f"{repeated} request id(s) appear in more than one attempt and would be billed twice")
    targets = {}
    for attempt in groups["accepted"]:
        targets.setdefault(attempt["target"], []).append(attempt)
    by_target = {}
    for target, attempts in sorted(targets.items()):
        summary = summarize(attempts, None)
        by_target[target] = {"attempts": summary["attempts"], "reviews": summary["reviews"], "items": summary["items"],
                             "cost_usd": summary["cost_usd"], "cost_per_review_usd": summary["cost_per_review_usd"],
                             "source_share_of_cost": summary["exploration_estimate"]["source_share_of_cost"]}
    report = {"schema_version": 1,
              "inputs": {"archives": str(archives.relative_to(root)),
                         "completion": {"path": str(completion_path.relative_to(root)),
                                        "sha256": sha256(completion_path.read_bytes())}},
              "accepted": summarize(groups["accepted"], claims), "accepted_by_target": by_target,
              "failed": {**summarize(groups["failed"], None), "reasons": dict(sorted(reasons.items()))},
              "requests_repeated_across_attempts": repeated}
    report["retry_cost_share"] = round(report["failed"]["cost_usd"] / (
        report["failed"]["cost_usd"] + report["accepted"]["cost_usd"]), 4) if report["accepted"]["cost_usd"] else None
    return report, problems


def money(value) -> str:
    return "unavailable" if value is None else f"${value:,.6f}"


def markdown(report: dict) -> str:
    accepted, failed = report["accepted"], report["failed"]
    lines = ["# Grading baseline", "",
             f"Archives: `{report['inputs']['archives']}`. Accepted attempts are those pinned by "
             f"`{report['inputs']['completion']['path']}` (sha256 `{report['inputs']['completion']['sha256']}`).", "",
             "## Coverage", "", "| | Accepted | Failed or replaced |", "| --- | ---: | ---: |"]
    for label, field in (("Archived attempts", "attempts"), ("With a session transcript", "attempts_with_transcript"),
                         ("With billed requests", "attempts_with_billed_requests"),
                         ("Without priced usage", "unpriced_attempts"), ("Reviews in billed attempts", "reviews"),
                         ("Items in billed attempts", "items"),
                         ("Transcript assistant lines", "transcript_lines"), ("Billed requests", "billed_requests"),
                         ("Tool calls", "tool_calls"), ("Requests without a tool call", "text_only_requests")):
        lines.append(f"| {label} | {accepted[field]:,} | {failed[field]:,} |")
    lines += ["", "Failed attempts by reason: " + "; ".join(f"{name} {count}" for name, count in failed["reasons"].items())
              + f". Request ids repeated across attempts: {report['requests_repeated_across_attempts']}.", "",
              "## Usage", "", "| Tokens | Accepted | Failed or replaced |", "| --- | ---: | ---: |"]
    for label, field in (("Fresh input", "input"), ("Cache write, 5 minutes", "cache_write_5m"),
                         ("Cache write, 1 hour", "cache_write_1h"), ("Cache write, tier unknown", "cache_write_unknown"),
                         ("Cache read", "cache_read"), ("Output, thinking included", "output"), ("Thinking", "thinking")):
        lines.append(f"| {label} | {accepted['tokens'][field]:,} | {failed['tokens'][field]:,} |")
    lines += ["", "| Cost | Accepted | Failed or replaced |", "| --- | ---: | ---: |"]
    for label, field in (("Fresh input", "fresh_input"), ("Cache writes", "cache_write"), ("Cache reads", "cache_read"),
                         ("Visible output", "visible_output"), ("Thinking", "thinking")):
        lines.append(f"| {label} | {money(accepted['cost_by_category_usd'][field])} | "
                     f"{money(failed['cost_by_category_usd'][field])} |")
    lines += [f"| Total | {money(accepted['cost_usd'])} | {money(failed['cost_usd'])} |", "",
              f"Retries are {report['retry_cost_share']:.1%} of the total. Accepted cost is "
              f"{money(accepted['cost_per_review_usd'])} per review, {money(accepted['cost_per_item_usd'])} per item and "
              f"{money(accepted['cost_per_claim_usd'])} per graded claim ({accepted['claims']:,} claims).", "",
              "## Dispatch time and reservations", ""]
    seconds, reservation = accepted["dispatch_seconds"], accepted["reservation_usd"]
    lines += [f"Accepted dispatches: median {seconds['median']:g} s, 90th percentile {seconds['p90']:g} s, longest "
              f"{seconds['max']:g} s, {seconds['sum']:g} s summed. Reservations: median {money(reservation['median'])}, "
              f"largest {money(reservation['max'])}; the costliest accepted session used "
              f"{accepted['peak_reservation_use']:.1%} of its reservation.", "",
              "## Exploration estimate", "", "| Turn's tool calls | Turns | Tool calls | Context growth (tokens) | "
              "Estimated carried cost | Share of accepted cost |", "| --- | ---: | ---: | ---: | ---: | ---: |"]
    for name, row in accepted["exploration_estimate"]["by_turn"].items():
        lines.append(f"| {name} | {row['turns']:,} | {row['tool_calls']:,} | {row['context_growth_tokens']:,} | "
                     f"{money(row['carried_cost_usd'])} | {row['carried_cost_usd'] / accepted['cost_usd']:.1%} |")
    estimate = accepted["exploration_estimate"]
    share = estimate["source_share_of_cost"]
    lines += ["", f"Source inspection is an estimated {share['low']:.1%} to {share['high']:.1%} of accepted cost: the "
              "lower figure counts turns that name only the pinned clone, the higher adds turns that also read prepared "
              "inputs and turns that name no known location. The initial context (prompt, tools and system text) "
              f"carried through each session is an estimated {money(estimate['initial_context_carried_cost_usd'])}, "
              f"{estimate['initial_context_carried_cost_usd'] / accepted['cost_usd']:.1%} of accepted cost. "
              f"{money(estimate['unattributed_cache_cost_usd'])} of recorded input and cache cost is unattributed. "
              f"Context shrank between requests {estimate['context_shrinks']} time(s); those steps are not attributed.",
              "", "## Accepted cost by target", "",
              "| Target | Attempts | Reviews | Items | Cost | Cost per review | Source share |",
              "| --- | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for target, row in report["accepted_by_target"].items():
        lines.append(f"| {target} | {row['attempts']:,} | {row['reviews']:,} | {row['items']:,} | {money(row['cost_usd'])} | "
                     f"{money(row['cost_per_review_usd'])} | {row['source_share_of_cost']['low']:.1%} to "
                     f"{row['source_share_of_cost']['high']:.1%} |")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--archives", type=Path, required=True)
    parser.add_argument("--completion", type=Path, required=True)
    parser.add_argument("--markdown", action="store_true")
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    try:
        report, problems = profile(args.archives.resolve(), args.completion.resolve(), args.root.resolve())
    except (OSError, ValueError, KeyError, tarfile.TarError) as error:
        print(f"grading_profile.py: {error}", file=sys.stderr)
        return 2
    if problems:
        print("\n".join(problems))
        return 1
    print(markdown(report) if args.markdown else json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
