#!/usr/bin/env python3
"""Meter saved Codex grading transcripts, including failed and replaced attempts."""

from __future__ import annotations

import argparse
from collections import Counter
from datetime import datetime
from decimal import Decimal
import hashlib
import json
from pathlib import Path
import sys
import tarfile
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "bench/tools"))
import codex_usage

COUNTS = ("turns", "tool_calls", "text_only_turns", "input", "cache_write", "cache_read", "output", "thinking")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def reference(path):
    return {"path": str(path), "sha256": digest(path.read_bytes())}


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def load_inputs(path, root):
    saved = read(path)
    pins = {"receipt": reference(path)}
    if "archive" not in saved:
        paths = sorted((path.parent / "home/.codex/sessions").rglob("rollout-*.jsonl"))
        return saved, [(str(p), p.read_bytes(), reference(p)) for p in paths], {
            **pins, "integrity": "direct file hashes captured at measurement"}
    archive = root / saved["archive"]["path"]
    archive_ref = reference(archive)
    if archive_ref["sha256"] != saved["archive"]["sha256"]:
        raise ValueError(f"archive hash differs: {archive}")
    expected = {row["path"]: row["sha256"] for row in saved["files"]}
    if len(expected) != len(saved["files"]):
        raise ValueError("receipt has repeated member paths")
    with tarfile.open(archive) as bundle:
        names = [member.name for member in bundle.getmembers()]
        selected = sorted(name for name in names if name.startswith("work/home/.codex/sessions/")
                          and Path(name).name.startswith("rollout-") and name.endswith(".jsonl"))

        def verified(name):
            if names.count(name) != 1 or name not in expected:
                raise ValueError(f"missing or repeated pinned archive member: {name}")
            member = bundle.getmember(name)
            if not member.isfile():
                raise ValueError(f"archive member is not a regular file: {name}")
            content = bundle.extractfile(member).read()
            if digest(content) != expected[name]:
                raise ValueError(f"archive member hash differs: {name}")
            return content, {"path": name, "sha256": expected[name]}

        dispatch_data, dispatch_ref = verified("work/dispatch.json")
        transcripts = [(name, *verified(name)) for name in selected]
    return json.loads(dispatch_data), transcripts, {
        **pins, "archive": archive_ref, "dispatch_member": dispatch_ref,
        "integrity": "archive and selected member hashes verified"}


def price(rollout, rate):
    total = {field: 0 for field in COUNTS}
    total["cost"] = 0.0
    for usage, calls in rollout["usage"]:
        input_rate, output_rate = rate["input"], rate["output"]
        if rate["model"] == "gpt-6.1-sol" and usage.get("input_tokens", 0) > 272000:
            input_rate, output_rate = input_rate * 2, output_rate * 1.5
        part = codex_usage.summarize({**rollout, "usage": [(usage, calls)]},
                                    (input_rate, output_rate), rate["cache_read"] / rate["input"],
                                    rate["cache_write_5m"] / rate["input"])
        for field in (*COUNTS, "cost"):
            total[field] += part[field]
    return total


def measure_attempt(path, root, disposition="unclassified"):
    record, transcripts, pins = load_inputs(path, root)
    rate = record["usage"]["rate"]
    if rate["model"] != record["model"]:
        raise ValueError("rate model differs from dispatch model")
    for field in ("input", "output", "cache_read", "cache_write_5m"):
        amount = Decimal(str(rate[field]))
        if not amount.is_finite() or amount < 0 or (field == "input" and amount == 0):
            raise ValueError(f"invalid rate: {field}")
    totals = {field: 0 for field in (*COUNTS, "recorded_tool_calls", "tool_turns")}
    totals["cost"] = 0.0
    rows, sessions, models, response_ids = [], {}, set(), set()
    with tempfile.TemporaryDirectory() as scratch:
        for index, (name, data, pin) in enumerate(transcripts):
            local = Path(scratch) / f"rollout-{index}.jsonl"
            local.write_bytes(data)
            rollout = codex_usage.read_rollout(local)
            session = rollout["meta"].get("id")
            if not session or session in sessions:
                raise ValueError(f"missing or repeated transcript session: {name}")
            sessions[session] = rollout["meta"].get("parent_thread_id")
            models.update(rollout["models"])
            calls = 0
            thread_ids = set()
            for line in data.decode("utf-8").splitlines():
                if not line.strip():
                    continue
                event = json.loads(line)
                payload = event.get("payload") or {}
                if event.get("type") == "response_item" and payload.get("type") in codex_usage.TOOL_TYPES:
                    calls += 1
                if event.get("type") == "token_usage_record" and payload.get("response_id"):
                    thread_ids.add(payload["response_id"])
            if response_ids & thread_ids:
                raise ValueError("a response ID appears in multiple transcripts")
            response_ids.update(thread_ids)
            part = {**price(rollout, rate), "recorded_tool_calls": calls,
                    "tool_turns": sum(count > 0 for _, count in rollout["usage"])}
            for field in totals:
                totals[field] += part[field]
            rows.append({"transcript": pin, "session_id": session, "parent_thread_id": sessions[session],
                         "observed": {key: value for key, value in part.items() if key != "cost"},
                         "observed_cost_usd": round(part["cost"], 6) if part["turns"] else None})
    if transcripts:
        roots = [session for session, parent in sessions.items() if not parent]
        if roots != [record["session_id"]]:
            raise ValueError("root transcript session differs from dispatch session")
        reachable = {record["session_id"]}
        while True:
            children = {session for session, parent in sessions.items() if parent in reachable}
            if children <= reachable:
                break
            reachable.update(children)
        if reachable != sessions.keys() or len(sessions) - 1 != record["subagents"]:
            raise ValueError("transcript lineage differs from dispatch subagent count")
    elif record.get("session_id"):
        raise ValueError("dispatch session has no saved transcript")
    if sorted(models) != record["models_observed"] or (models and models != {record["model"]}):
        raise ValueError("transcript model differs from dispatch model identity")
    cost = round(totals.pop("cost"), 6) if totals["turns"] else None
    recorded_cost = record["usage"].get("priced_total_usd")
    if recorded_cost is not None and cost != round(recorded_cost, 6):
        raise ValueError("observed cost differs from dispatch receipt at six decimal places")
    if record["usage"].get("requests") is not None and totals["turns"] != record["usage"]["requests"]:
        raise ValueError("recorded request count differs from dispatch receipt")
    duration = None
    if record.get("dispatched_at") and record.get("completed_at"):
        duration = (datetime.fromisoformat(record["completed_at"].replace("Z", "+00:00")) -
                    datetime.fromisoformat(record["dispatched_at"].replace("Z", "+00:00"))).total_seconds()
    failure = record.get("exit_code") != 0 or bool(record.get("audit_violations")) or not record.get("verdicts_present")
    if disposition == "accepted" and failure:
        raise ValueError("accepted attempt has a failed dispatch or no verdicts")
    if disposition == "unclassified" and failure:
        disposition = "failed"
    return {"inputs": pins, "disposition": disposition, "session_id": record.get("session_id"),
            "response_ids": sorted(response_ids), "model": record["model"], "effort": record.get("effort"),
            "cli_version": record.get("cli_version"), "rate": rate, "threads": rows,
            "observed": {**totals, "unattributed_tool_calls": totals["recorded_tool_calls"] - totals["tool_calls"]},
            "observed_cost_usd": cost, "recorded_usage": record["usage"],
            "charge_upper_usd": record["usage"].get("high"),
            "charge_upper_unknown": record["usage"].get("high") is None,
            "exit_code": record.get("exit_code"), "audit_violations": record.get("audit_violations"),
            "dispatch_seconds": duration}


def summarize(attempts):
    fields = (*COUNTS, "recorded_tool_calls", "tool_turns", "unattributed_tool_calls")
    costs = [row["observed_cost_usd"] for row in attempts if row["observed_cost_usd"] is not None]
    upper = [row["charge_upper_usd"] for row in attempts]
    return {"attempts": len(attempts), "observed": {field: sum(row["observed"][field] for row in attempts)
                                                 for field in fields},
            "known_observed_cost_usd_sum": float(sum((Decimal(str(cost)) for cost in costs), Decimal(0))),
            "attempts_with_observed_cost": len(costs),
            "attempts_without_observed_cost": len(attempts) - len(costs),
            "unknown_charge_upper_attempts": sum(value is None for value in upper),
            "charge_upper_usd": float(sum((Decimal(str(value)) for value in upper), Decimal(0)))
            if attempts and all(value is not None for value in upper) else None,
            "dispatch_seconds_known_sum": sum(row["dispatch_seconds"] for row in attempts
                                              if row["dispatch_seconds"] is not None),
            "attempts_without_dispatch_seconds": sum(row["dispatch_seconds"] is None for row in attempts)}


def report(inputs, root):
    attempts = [measure_attempt(path, root, disposition) for path, disposition in inputs]
    sessions = Counter(row["session_id"] for row in attempts if row["session_id"])
    responses = Counter(identity for row in attempts for identity in row["response_ids"])
    if any(count > 1 for count in sessions.values()) or any(count > 1 for count in responses.values()):
        raise ValueError("repeated session or response across attempt inputs")
    return {"schema_version": 1, "meter": reference(ROOT / "bench/tools/codex_usage.py"), "attempts": attempts,
            "raw_total": summarize(attempts),
            "by_disposition": {name: summarize([row for row in attempts if row["disposition"] == name])
                               for name in ("accepted", "failed", "unclassified")},
            "limits": ["Totals count saved usage records only; absent request usage remains unknown.",
                       "Observed cost is list-price equivalent, not an account invoice or quota measure.",
                       "Tool calls after the final usage record are recorded but have no billed-turn attribution.",
                       "Dispatch seconds come from receipts; controller wall time is outside this meter."]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("receipts", type=Path, nargs="*")
    parser.add_argument("--accepted", type=Path, action="append", default=[])
    parser.add_argument("--failed", type=Path, action="append", default=[])
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    inputs = [(path, "unclassified") for path in args.receipts]
    inputs += [(path, "accepted") for path in args.accepted] + [(path, "failed") for path in args.failed]
    if not inputs:
        parser.error("supply evidence.json or work/dispatch.json receipts")
    try:
        result = report(inputs, args.root)
    except (OSError, ValueError, KeyError, TypeError, tarfile.TarError) as error:
        print(f"measure.py: {error}", file=sys.stderr)
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
