#!/usr/bin/env python3
"""Write the usage of the round's Codex sessions from their logs.

    python3 docs/research/older-problem-activation-2026-10-13/usage.py LOGS

LOGS holds one `<name>.jsonl` log of `codex exec --json` for each session. A name that ends in `-astra` ran
gpt-6-astra and one that ends in `-sol` ran gpt-6.1-sol, as run.sh sets it. Prices are the list-price equivalent at
the latest rates of bench/rates.current.json; the sessions ran on the ChatGPT plan."""
import hashlib
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
CASES = {"x1": "OP-candidate-x-1", "s1": "OP-candidate-s-1", "s2": "OP-candidate-s-2", "s3": "OP-candidate-s-3", "o1": "OP-candidate-o-1"}


def rate(model):
    rows = json.loads((ROOT / "bench/rates.current.json").read_text(encoding="utf-8"))["rates"]
    return max((row for row in rows if row["model"] == model), key=lambda row: row["as_of"])


if __name__ == "__main__":
    sessions = []
    for log in sorted(Path(sys.argv[1]).glob("*.jsonl")):
        usage, thread = None, None
        for line in log.read_text(encoding="utf-8").splitlines():
            event = json.loads(line)
            thread, usage = event.get("thread_id", thread), event.get("usage", usage)
        name = log.name.removesuffix(".jsonl")
        model = "gpt-6-astra" if name.endswith("-astra") else "gpt-6.1-sol"
        price, uncached = rate(model), usage["input_tokens"] - usage["cached_input_tokens"]
        sessions.append({"session": name, "case": CASES[name.split("-")[0]], "model": model, "effort": "high", "thread_id": thread,
                         "uncached_input_tokens": uncached, "cached_input_tokens": usage["cached_input_tokens"], "output_tokens": usage["output_tokens"],
                         "list_price_usd": round((uncached * price["input"] + usage["cached_input_tokens"] * price["cache_read"]
                                                  + usage["output_tokens"] * price["output"]) / 1e6, 4),
                         "log_sha256": hashlib.sha256(log.read_bytes()).hexdigest()})
    total = round(sum(session["list_price_usd"] for session in sessions), 2)
    (HERE / "usage.v1.json").write_text(json.dumps({
        "schema_version": 1,
        "billing": "list-price equivalent at the latest rates of bench/rates.current.json; the sessions ran on the ChatGPT plan",
        "limits": {"sessions": 30, "list_price_usd": 15},
        "sessions": sessions, "total_list_price_usd": total}, indent=1) + "\n", encoding="utf-8")
    print(len(sessions), "sessions", total)
