#!/usr/bin/env python3
"""Dispatch the next cells of one Codex run one at a time, stopping at the first review that is not valid.

Usage::

    python3 docs/research/skill-matrix-2026-10-02/dispatch_serial.py RUN [--count N] [--weekly-stop PERCENT]

A run's ``max_in_flight`` cap is per run, so nothing in ``run_cell.py`` keeps two runs, or two
callers, from reviewing at once. This holds a lock for its whole life and refuses to start while
any matrix run has an attempt in flight. Each review is provisioned, dispatched, audited, filed and
cleaned up by ``run_cell.py --next`` before the next one starts.

It stops, without dispatching again, when a review files as anything but ``valid completed`` (so
the failure is diagnosed before more usage is spent), when the ChatGPT plan's weekly usage in the
last review's sessions has reached ``--weekly-stop`` (default 95), or after ``--count`` reviews
(default 1). One JSON line per review goes to stdout.
"""

import argparse
import fcntl
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "bench/tools"
WORK_ROOT = Path.home() / ".t3/bench-runs"
REPLACEMENTS = [ROOT / "docs/research/selected-cache-rebuild-2026-10-02/cache-replacements.v1.json",
                ROOT / "docs/research/original-cache-rebuild-2026-10-02/cache-replacements.v1.json"]
sys.path.insert(0, str(TOOLS))
import run_cell  # noqa: E402


def weekly_used(attempt: Path):
    """The last weekly usage percentage the client reported in the attempt's sessions, or None."""
    latest = None
    for path in (attempt / "home/.codex/sessions").rglob("rollout-*.jsonl"):
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            if '"rate_limits"' not in line:
                continue
            record = json.loads(line)
            primary = ((record.get("payload") or {}).get("rate_limits") or {}).get("primary")
            if primary and (latest is None or record["timestamp"] > latest[0]):
                latest = (record["timestamp"], primary["used_percent"])
    return latest and latest[1]


def in_flight():
    busy = []
    for directory in sorted((ROOT / "bench/runs").glob("2026-10-0[23]-*")):
        if (WORK_ROOT / directory.name).is_dir():
            busy += [f"{directory.name}/{attempt}" for attempt in run_cell.Run(directory, WORK_ROOT / directory.name).in_flight()]
    return busy


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("run")
    parser.add_argument("--count", type=int, default=1)
    parser.add_argument("--weekly-stop", type=float, default=95.0)
    args = parser.parse_args()
    run_dir = Path(args.run).resolve(strict=True)
    work = WORK_ROOT / run_dir.name
    WORK_ROOT.mkdir(parents=True, exist_ok=True)
    lock = open(WORK_ROOT / "serial.lock", "a", encoding="utf-8")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        raise SystemExit("another serial dispatcher is running")
    env = dict(os.environ, BENCH_RATES=str(ROOT / "bench/rates.current.json"),
               BENCH_ARCHIVE_ROOT=str(ROOT / "artifacts/transcripts"), BENCH_CACHE_ROOT=str(Path.home() / ".t3/bench-cache"),
               BENCH_CACHE_REPLACEMENTS=os.pathsep.join(str(path) for path in REPLACEMENTS))
    used = None
    for _ in range(args.count):
        busy = in_flight()
        if busy:
            raise SystemExit(f"attempts in flight: {', '.join(busy)}")
        if used is not None and used >= args.weekly_stop:
            print(json.dumps({"stopped": f"weekly usage {used}% reached the {args.weekly_stop}% stop"}))
            return 0
        before = {path.name for path in work.glob("att-*")}
        quota = f"ChatGPT plan, weekly usage {'unknown' if used is None else f'{used}%'} before dispatch; serial dispatch"
        done = subprocess.run([sys.executable, str(TOOLS / "run_cell.py"), "--run", str(run_dir), "--work", str(work),
                               "--next", "--quota", quota], cwd=ROOT, env=env, capture_output=True, text=True)
        new = sorted({path.name for path in work.glob("att-*")} - before)
        if not new:
            print(json.dumps({"stopped": "nothing dispatched", "exit": done.returncode, "output": (done.stdout + done.stderr)[-600:]}))
            return 0 if "every cell in the sealed order has an attempt" in done.stdout + done.stderr else 1
        record_path = run_dir / "attempts" / new[0] / "attempt.json"
        if not record_path.is_file():
            print(json.dumps({"stopped": f"{new[0]} was dispatched but not filed", "exit": done.returncode,
                              "output": (done.stdout + done.stderr)[-600:]}))
            return 1
        record = json.loads(record_path.read_text(encoding="utf-8"))
        used = weekly_used(work / new[0])
        print(json.dumps({"attempt": new[0], "cell": record["cell"], "disposition": record["disposition"],
                          "usd": record["usage"].get("priced_total_usd"), "dispatched_at": record["timing"]["dispatched_at"],
                          "ended_at": record["timing"].get("completed_at") or record["timing"].get("stopped_at"),
                          "weekly_used_percent": used}), flush=True)
        if record["disposition"] != "valid completed":
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
