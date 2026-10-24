#!/usr/bin/env python3
"""Reprice each filed attempt of the given runs at Opus's and Fable's list rates, from its usage-requests.jsonl.

Prints, per run, totals and per-review figures and, per task, each attempt's Fable-priced cost, output tokens and
minutes from dispatch to completion or stop.
"""
from datetime import datetime
import json
from pathlib import Path
import statistics
import sys

# input, output, cache read, 5-minute and 1-hour cache writes, dollars per million tokens (bench/rates.current.json)
RATES = {"opus": (4, 20, 0.2, 5, 8), "fable": (10, 50, 0.25, 12.5, 20)}


def price(request, rates):
    base, output, read, write_5m, write_1h = rates
    unknown = request["cache_creation_input_tokens"] - request.get("cache_write_5m", 0) - request.get("cache_write_1h", 0)
    return (request["input_tokens"] * base + request["output_tokens"] * output
            + request["cache_read_input_tokens"] * read + request.get("cache_write_5m", 0) * write_5m
            + request.get("cache_write_1h", 0) * write_1h + max(0, unknown) * write_5m) / 1e6


def stamp(text):
    return datetime.fromisoformat(text.replace("Z", "+00:00"))


def attempts(run):
    for directory in sorted(Path(run, "attempts").iterdir()):
        record = json.loads((directory / "attempt.json").read_text())
        usage = directory / "usage-requests.jsonl"
        requests = [json.loads(line) for line in usage.read_text().splitlines()] if usage.exists() else []
        timing = record["timing"]
        end = timing.get("completed_at") or timing.get("stopped_at")
        yield {"target": record["cell"]["target"], "disposition": record["disposition"],
               "filed_usd": record["usage"]["priced_total_usd"],
               **{name: sum(price(request, rates) for request in requests) for name, rates in RATES.items()},
               "output_tokens": sum(request["output_tokens"] for request in requests),
               "cache_read_tokens": sum(request["cache_read_input_tokens"] for request in requests),
               "minutes": (stamp(end) - stamp(timing["dispatched_at"])).total_seconds() / 60 if end else None}


def main(runs):
    for run in runs:
        rows = list(attempts(run))
        print(f"== {Path(run).name}: {len(rows)} attempts")
        for key in ("filed_usd", "opus", "fable", "minutes", "output_tokens", "cache_read_tokens"):
            values = [row[key] for row in rows if row[key] is not None]
            print(f"  {key}: total {sum(values):.2f}, mean {statistics.mean(values):.2f}, "
                  f"median {statistics.median(values):.2f}, max {max(values):.2f}, n {len(values)}")
        valid = sum(row["disposition"] == "valid completed" for row in rows)
        print(f"  valid completed: {valid}")
        by_target = {}
        for row in rows:
            by_target.setdefault(row["target"], []).append(row)
        for target, group in sorted(by_target.items()):
            print(f"   {target}: fable $" + ", ".join(f"{row['fable']:.2f}" for row in group)
                  + " | output " + ", ".join(f"{row['output_tokens'] / 1e3:.0f}k" for row in group)
                  + " | minutes " + ", ".join(f"{row['minutes']:.0f}" for row in group if row["minutes"]))


if __name__ == "__main__":
    main(sys.argv[1:])
