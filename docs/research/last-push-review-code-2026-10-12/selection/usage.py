#!/usr/bin/env python3
"""Write the usage of the matching sessions from their logs.

    python3 docs/research/last-push-review-code-2026-10-12/selection/usage.py LOGS CLIENT

LOGS holds one `<session>.run.jsonl` log of `codex exec --json` per session, and CLIENT is the output of
`codex --version`. Every session ran gpt-6.1-sol at high reasoning effort. Prices are the list-price equivalent at the
latest rate of bench/rates.current.json; the sessions ran on the ChatGPT plan. The result is `usage.v1.json` beside
this file. It follows `docs/research/last-push-rulings-2026-10-11/usage.py`."""
import hashlib
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
MODEL = "gpt-6.1-sol"
TOKENS = {"uncached_input_tokens": "input", "cached_input_tokens": "cache_read", "output_tokens": "output"}


def main():
    logs, client = sys.argv[1:]
    rates = json.loads((ROOT / "bench/rates.current.json").read_text(encoding="utf-8"))["rates"]
    rate = max((row for row in rates if row["model"] == MODEL), key=lambda row: row["as_of"])
    sessions = []
    for log in sorted(Path(logs).glob("*.run.jsonl")):
        usage, thread = None, None
        for line in log.read_text(encoding="utf-8").splitlines():
            event = json.loads(line)
            thread, usage = event.get("thread_id", thread), event.get("usage", usage)
        tokens = {"uncached_input_tokens": usage["input_tokens"] - usage["cached_input_tokens"],
                  "cached_input_tokens": usage["cached_input_tokens"], "output_tokens": usage["output_tokens"]}
        sessions.append({"session": log.name.removesuffix(".run.jsonl"), "thread_id": thread, **tokens,
                         "list_price_usd": round(sum(tokens[name] * rate[price] for name, price in TOKENS.items()) / 1e6, 4),
                         "log_sha256": hashlib.sha256(log.read_bytes()).hexdigest()})
    total = {name: sum(session[name] for session in sessions) for name in TOKENS}
    document = {"schema_version": 1, "model": MODEL, "effort": "high", "client": client,
                "billing": f"list-price equivalent at the {MODEL} rates of bench/rates.current.json; the sessions ran on the ChatGPT plan",
                "rates_usd_per_million": {price: rate[price] for price in TOKENS.values()}, "sessions": sessions,
                "total": {**total, "list_price_usd": round(sum(session["list_price_usd"] for session in sessions), 4)}}
    (HERE / "usage.v1.json").write_text(json.dumps(document, indent=1) + "\n", encoding="utf-8")
    print(len(sessions), "sessions", document["total"]["list_price_usd"])


if __name__ == "__main__":
    main()
