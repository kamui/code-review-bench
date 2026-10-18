#!/usr/bin/env python3
"""Read, summarize and serialize full rollouts for local process measurements."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from bench.tools import codex_usage


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", choices=("existing", "typed"), required=True)
    parser.add_argument("path", type=Path)
    parser.add_argument("input_price", type=float)
    parser.add_argument("output_price", type=float)
    parser.add_argument("cached_mult", type=float)
    parser.add_argument("write_mult", type=float)
    parser.add_argument("repeat", type=int, nargs="?", default=1)
    parser.add_argument("warmup", type=int, nargs="?", default=0)
    args = parser.parse_args(argv)
    if args.repeat < 1 or args.warmup < 0:
        parser.error("repeat must be positive and warmup must be non-negative")
    prices = (args.input_price, args.output_price)
    if args.engine == "typed":
        from tools.usage_pilot import typed_usage

    def iteration():
        if args.engine == "existing":
            return codex_usage.summarize(codex_usage.read_rollout(args.path), prices,
                                         args.cached_mult, args.write_mult)
        return typed_usage.summarize(typed_usage.read_rollout(args.path), prices,
                                      args.cached_mult, args.write_mult).to_dict()

    try:
        for _ in range(args.warmup):
            json.dumps(iteration(), allow_nan=False)
        byte_length = args.path.stat().st_size
    except (OSError, ValueError, TypeError, OverflowError) as error:
        print(json.dumps({"kind": "input", "detail": str(error)}), file=sys.stderr)
        return 2

    completed = 0
    elapsed_start = time.perf_counter()
    cpu_start = time.process_time()
    status = 0
    failure = None
    try:
        for _ in range(args.repeat):
            summary = iteration()
            print(json.dumps(summary, allow_nan=False), flush=True)
            completed += 1
    except (OSError, ValueError, TypeError, OverflowError) as error:
        status = 2
        failure = {"kind": "input", "detail": str(error)}
    elapsed = time.perf_counter() - elapsed_start
    cpu = time.process_time() - cpu_start
    if failure is not None:
        print(json.dumps(failure), file=sys.stderr)
    print(json.dumps({
        "elapsed_seconds": elapsed, "cpu_seconds": cpu,
        "completed_iterations": completed, "bytes_processed": byte_length * completed,
    }), file=sys.stderr)
    return status


if __name__ == "__main__":
    sys.exit(main())
