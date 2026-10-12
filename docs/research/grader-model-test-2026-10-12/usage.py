#!/usr/bin/env python3
"""Count and price the requests of saved grader sessions and write `usage.v1.json`.

    python3 docs/research/grader-model-test-2026-10-12/usage.py SCRATCH SAVED_ATTEMPTS

SCRATCH is this test's scratch directory and SAVED_ATTEMPTS the trial's (`.local/trial/attempts` of the worktree that
ran it); both hold each session's client transcript. One request is the transcript lines that share a `requestId`:
its input and cache counts are taken once and its output count is the largest recorded, the rule of
`bench/tools/transcript_usage.py`. Each request is priced on its own from `RATES`, the provider's page as read on
2026-10-12. Claude Haiku 5.5 has two sets of prices: a request whose prompt (input, cache reads and cache writes) is
over 100,000 tokens pays the higher set. Run from the repository root."""
import json
import sys
from datetime import datetime
from pathlib import Path

HERE = Path(__file__).resolve().parent
KINDS = ("input", "cache_write_5m", "cache_write_1h", "cache_read", "output")
SOURCE = ("https://docs.anthropic.com/en/docs/about-claude/pricing.md read 2026-10-12, sha256 "
          "67904e280b595d31f373c857844ed380072ae79d280d8f385640b5992d337ca9; dollars per million tokens")
# (prompt tokens above which the row applies, prices)
RATES = {
    "claude-opus-5-5": [(0, {"input": 4.0, "cache_write_5m": 5.0, "cache_write_1h": 8.0, "cache_read": 0.2, "output": 20.0})],
    "claude-sonnet-5-5": [(0, {"input": 2.0, "cache_write_5m": 2.5, "cache_write_1h": 4.0, "cache_read": 0.1, "output": 10.0})],
    "claude-haiku-5-5": [(0, {"input": 0.1, "cache_write_5m": 0.125, "cache_write_1h": 0.2, "cache_read": 0.01, "output": 0.5}),
                         (100_000, {"input": 0.5, "cache_write_5m": 0.625, "cache_write_1h": 1.0, "cache_read": 0.05, "output": 2.5})],
}
GRADING_TOOLS = ("inspect", "run", "write_verdicts", "edit_verdicts", "write_scratch", "validate")


def requests(transcripts):
    """One row per API request of the transcripts: its tokens by kind, its model and its tool calls."""
    rows = {}
    for path in transcripts:
        for number, line in enumerate(Path(path).read_text(encoding="utf-8").splitlines()):
            try:
                record = json.loads(line)
            except ValueError:
                continue
            message = record.get("message") or {}
            if record.get("type") != "assistant" or message.get("model") == "<synthetic>" or record.get("isApiErrorMessage"):
                continue
            usage, split = message.get("usage") or {}, (message.get("usage") or {}).get("cache_creation") or {}
            row = rows.setdefault(record.get("requestId") or f"{path}:{number}", {
                "model": message.get("model"), "tools": {}, "output": 0,
                "input": usage.get("input_tokens", 0), "cache_read": usage.get("cache_read_input_tokens", 0),
                "cache_write_1h": split.get("ephemeral_1h_input_tokens", 0),
                # A write with no recorded lifetime is priced as a five-minute write, as transcript_usage.py does.
                "cache_write_5m": usage.get("cache_creation_input_tokens", 0) - split.get("ephemeral_1h_input_tokens", 0)})
            row["output"] = max(row["output"], usage.get("output_tokens", 0))
            for block in message.get("content") or []:
                if isinstance(block, dict) and block.get("type") == "tool_use":
                    row["tools"][block["id"]] = block["name"].removeprefix("mcp__grading__")
    return list(rows.values())


def refused_validations(transcripts):
    """How many `validate` calls of the transcripts reported a violation or an error."""
    calls, refused = set(), 0
    for path in transcripts:
        for line in Path(path).read_text(encoding="utf-8").splitlines():
            try:
                record = json.loads(line)
            except ValueError:
                continue
            content = (record.get("message") or {}).get("content")
            for block in content if isinstance(content, list) else []:
                if block.get("type") == "tool_use" and block["name"] == "mcp__grading__validate":
                    calls.add(block["id"])
                elif block.get("type") == "tool_result" and block.get("tool_use_id") in calls:
                    calls.discard(block["tool_use_id"])
                    text = "".join(part.get("text", "") for part in block["content"]) if isinstance(block["content"], list) else block["content"]
                    try:
                        result = json.loads(text)
                        refused += bool(result["exit_code"] or result["violations"] or result["error"])
                    except (ValueError, KeyError, TypeError):
                        refused += 1
    return refused


def price(row):
    prompt = row["input"] + row["cache_read"] + row["cache_write_5m"] + row["cache_write_1h"]
    rate = [prices for above, prices in RATES[row["model"]] if prompt > above or above == 0][-1]
    return sum(row[kind] * rate[kind] for kind in KINDS) / 1e6


def session(work):
    """The usage of the session saved in one grading workspace, or None when it left no transcript."""
    work = Path(work)
    transcripts = sorted((work / "home/.claude/projects").glob("*/*.jsonl"))
    rows = requests(transcripts)
    if not rows:
        return None
    record = json.loads((work / "dispatch.json").read_text(encoding="utf-8")) if (work / "dispatch.json").exists() else {}
    tools = [name for row in rows for name in row["tools"].values()]
    seconds = None
    if record.get("dispatched_at") and record.get("completed_at"):
        seconds = int((datetime.fromisoformat(record["completed_at"].replace("Z", "+00:00"))
                       - datetime.fromisoformat(record["dispatched_at"].replace("Z", "+00:00"))).total_seconds())
    return {"session_id": record.get("session_id"), "models": sorted({row["model"] for row in rows}),
            "cli_version": record.get("cli_version"), "exit_code": record.get("exit_code"),
            "dispatched_at": record.get("dispatched_at"), "completed_at": record.get("completed_at"), "seconds": seconds,
            "requests": len(rows),
            "requests_over_100k_prompt": sum(row["input"] + row["cache_read"] + row["cache_write_5m"] + row["cache_write_1h"] > 100_000
                                             for row in rows),
            "tokens": {kind: sum(row[kind] for row in rows) for kind in KINDS},
            "tool_calls": {name: tools.count(name) for name in GRADING_TOOLS if name in tools},
            "other_tool_calls": sum(name not in GRADING_TOOLS for name in tools),
            "refused_validations": refused_validations(transcripts),
            "list_price_usd": round(sum(price(row) for row in rows), 6),
            "dispatch_record_usd": (record.get("usage") or {}).get("priced_total_usd")}


def main(scratch, saved_attempts):
    scratch, saved_attempts = Path(scratch), Path(saved_attempts)
    batches = json.loads((HERE / "batches.json").read_text(encoding="utf-8"))["batches"]
    trial = HERE.parent / "cohort-rebuild-2026-10-05/trial"
    by_session = {}
    for attempt in [*scratch.glob("attempts/*/*/*/*"), *saved_attempts.glob("*/*/*"), *scratch.glob("probe/*")]:
        record = attempt / "work/dispatch.json"
        if record.exists():
            by_session[json.loads(record.read_text(encoding="utf-8"))["session_id"]] = attempt
    sessions, attempts = [], []
    for batch in batches:
        name = f"{Path(batch['run']).name}/{batch['target']}"
        for grader, directory in (("claude-opus-5-5", trial / batch["opus"]), ("claude-sonnet-5-5", HERE / "results/claude-sonnet-5-5" / name),
                                  ("claude-haiku-5-5", HERE / "results/claude-haiku-5-5" / name)):
            if not (directory / "dispatch.json").exists():
                continue
            saved = json.loads((directory / "dispatch.json").read_text(encoding="utf-8"))
            attempt = by_session.get(saved["session_id"])
            measured = session(attempt / "work") if attempt else None
            sessions.append({"grader": grader, "run": batch["run"], "target": batch["target"], "set": batch["set"],
                             "reviews": batch["reviews"], "comments": batch["comments"],
                             **(measured or {"session_id": saved["session_id"], "transcript": "not found",
                                             "dispatch_record_usd": saved["usage"]["priced_total_usd"]})})
    saved_sessions = {row["session_id"] for row in sessions}
    for attempt in sorted([*scratch.glob("attempts/*/*/*/*"), *scratch.glob("probe/*")]):
        measured = session(attempt / "work") if (attempt / "work").exists() else None
        if measured and measured["session_id"] not in saved_sessions:
            attempts.append({"attempt": str(attempt.relative_to(scratch)), **measured})
    document = {"schema_version": 1, "rates": {"source": SOURCE, "models": {
        model: [{"prompt_tokens_over": above, **prices} for above, prices in rows] for model, rows in RATES.items()}},
        "sessions": sessions, "sessions_without_a_saved_result": attempts}
    (HERE / "usage.v1.json").write_text(json.dumps(document, indent=1) + "\n", encoding="utf-8")
    for grader in RATES:
        rows = [row for row in sessions if row["grader"] == grader and "list_price_usd" in row]
        print(grader, len(rows), "sessions", round(sum(row["list_price_usd"] for row in rows), 2), "USD at list price")


if __name__ == "__main__":
    main(*sys.argv[1:])
