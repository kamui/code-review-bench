#!/usr/bin/env python3
"""Write the usage of the round's Codex sessions from their logs.

    python3 docs/research/last-push-rulings-2026-10-11/usage.py LOGS [LOGS ...]

Each LOGS directory holds `<name>.jsonl` or `<name>.run.jsonl` logs of `codex exec --json`. The model of a session
is read from its name: a name that contains `astra` ran gpt-6-astra, every other one gpt-6.1-sol. Prices are the
list-price equivalent at the latest rates of bench/rates.current.json; the sessions ran on the ChatGPT plan."""
import hashlib
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def rate(model):
    return max((row for row in json.loads((ROOT / "bench/rates.current.json").read_text(encoding="utf-8"))["rates"] if row["model"] == model),
               key=lambda row: row["as_of"])


def main(directories):
    sessions = []
    for directory in directories:
        for log in sorted(Path(directory).glob("*.jsonl")):
            usage, thread = None, None
            for line in log.read_text(encoding="utf-8").splitlines():
                event = json.loads(line)
                thread = event.get("thread_id", thread)
                usage = event.get("usage", usage)
            model = "gpt-6-astra" if "astra" in log.name else "gpt-6.1-sol"
            price = rate(model)
            uncached = usage["input_tokens"] - usage["cached_input_tokens"]
            sessions.append({"session": log.name.removesuffix(".jsonl").removesuffix(".run"), "model": model, "effort": "high", "thread_id": thread,
                             "uncached_input_tokens": uncached, "cached_input_tokens": usage["cached_input_tokens"], "output_tokens": usage["output_tokens"],
                             "list_price_usd": round((uncached * price["input"] + usage["cached_input_tokens"] * price["cache_read"]
                                                      + usage["output_tokens"] * price["output"]) / 1e6, 4),
                             "log_sha256": hashlib.sha256(log.read_bytes()).hexdigest()})
    total = round(sum(session["list_price_usd"] for session in sessions), 2)
    (HERE / "usage.v1.json").write_text(json.dumps({"schema_version": 1, "billing": "list-price equivalent at the latest rates of bench/rates.current.json; the sessions ran on the ChatGPT plan",
                                                    "sessions": sessions, "total_list_price_usd": total}, indent=1) + "\n", encoding="utf-8")
    print(len(sessions), "sessions", total)


if __name__ == "__main__":
    main(sys.argv[1:])
