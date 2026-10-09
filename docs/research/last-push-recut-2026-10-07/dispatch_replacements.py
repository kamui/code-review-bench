#!/usr/bin/env python3
"""Dispatch the frozen built-in replacements serially; stop on failure, quota or incomplete cleanup."""
import argparse
import fcntl
import json
from pathlib import Path
import select
import subprocess
import sys
import time
import urllib.request

import replacement_runs as queue


def meters(env):
    credentials = queue.read(Path.home() / ".claude/.credentials.json")
    request = urllib.request.Request("https://api.anthropic.com/api/oauth/usage", headers={
        "Authorization": "Bearer " + credentials["claudeAiOauth"]["accessToken"],
        "anthropic-beta": "oauth-2025-04-20", "User-Agent": "claude-code/2.1.294"})
    with urllib.request.urlopen(request, timeout=30) as response:
        usage = json.load(response)
    claude = {key: usage.get(key) for key in ("five_hour", "seven_day")}
    process = subprocess.Popen([env["BENCH_CODEX"], "app-server"], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1, env=env)
    try:
        def send(row):
            process.stdin.write(json.dumps(row) + "\n")
            process.stdin.flush()

        def receive(identifier):
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                if not select.select([process.stdout], [], [], 1)[0]:
                    continue
                line = process.stdout.readline()
                if not line:
                    raise RuntimeError("app-server closed before returning usage")
                row = json.loads(line)
                if row.get("id") == identifier:
                    if "error" in row:
                        raise RuntimeError("app-server could not return usage")
                    return row["result"]
            raise TimeoutError("app-server usage timeout")

        send({"id": 1, "method": "initialize", "params": {"clientInfo": {"name": "bench-meter", "version": "1.0"}}})
        receive(1)
        send({"method": "initialized", "params": {}})
        send({"id": 2, "method": "account/rateLimits/read", "params": {}})
        result = receive(2)
        codex = {key: result.get(key) for key in ("ordinaryUsageAllowed", "rateLimits")}
    finally:
        process.terminate()
        process.wait(timeout=10)
    return {"at": queue.now(), "claude": claude, "codex": codex}


def stop_for_quota(usage, threshold):
    for window in usage["claude"].values():
        if window and window["utilization"] >= threshold:
            return "Claude usage reached the quota stop"
    codex = usage["codex"]
    if not codex["ordinaryUsageAllowed"]:
        return "ChatGPT ordinary usage is unavailable"
    for key in ("primary", "secondary"):
        window = codex["rateLimits"].get(key)
        if window and window["usedPercent"] >= threshold:
            return "ChatGPT usage reached the quota stop"
    return None


def ready(runs):
    for run in runs:
        queue.run_cell.check_frozen(run)
        checked = subprocess.run(["git", "diff", "--quiet", run.manifest["freeze_commit"], "--",
                                  "bench/tools", "bench/schema", "bench/policies", "docs/clean-context.md",
                                  str(Path(__file__).relative_to(queue.ROOT)),
                                  str(Path(queue.__file__).relative_to(queue.ROOT))], cwd=queue.ROOT)
        if checked.returncode:
            raise SystemExit(f"runner runtime differs from the freeze commit: {run.id}")
        if run.in_flight():
            raise SystemExit(f"attempts in flight: {run.id}: {run.in_flight()}")
        for attempt_id, record in run.filed.items():
            workspace = run.work / attempt_id
            if record["disposition"] != "valid completed":
                successors = [row for row in run.filed.values() if row.get("predecessor") == attempt_id]
                if not successors:
                    raise SystemExit(f"failure needs investigation before resuming: {run.id}/{attempt_id}")
            elif workspace.exists():
                receipt = workspace / "workspace-pruned.json"
                if not receipt.is_file() or queue.read(receipt).get("applied") is not True or any(
                    (workspace / name).exists() or (workspace / name).is_symlink() for name in ("clone", "clone-cache")
                ):
                    raise SystemExit(f"cleanup incomplete: {run.id}/{attempt_id}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--count", type=int, default=170)
    parser.add_argument("--quota-stop", type=float, default=95)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    queue.check_queue()
    env = queue.environment()
    work_root = Path.home() / ".t3/bench-runs"
    work_root.mkdir(parents=True, exist_ok=True)
    with (work_root / "serial.lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise SystemExit("another serial dispatcher is running")
        usage = meters(env)
        print(json.dumps({"meters": usage}), flush=True)
        if args.check:
            ready([queue.run_cell.Run(queue.directory(spec[0]), work_root / queue.directory(spec[0]).name)
                   for spec in queue.SPECS])
            print(json.dumps({"ready": True, "quota_stop": stop_for_quota(usage, args.quota_stop)}), flush=True)
            return 0
        next_meter = time.monotonic() + 3600
        for _ in range(args.count):
            runs = [queue.run_cell.Run(queue.directory(spec[0]), work_root / queue.directory(spec[0]).name)
                    for spec in queue.SPECS]
            ready(runs)
            for run in runs:
                if queue.read(run.dir / "clients.frozen.json") != queue.read(queue.ROOT / ".local/issue60/clients.json"):
                    raise SystemExit(f"client pins differ: {run.id}")
            stop = stop_for_quota(usage, args.quota_stop)
            if stop:
                print(json.dumps({"stopped": stop, "meters": usage}), flush=True)
                return 0
            pending = [run for run in runs if queue.run_cell.status(run)["next_cell"]]
            if not pending:
                print(json.dumps({"completed": True, "status": [queue.run_cell.status(run) for run in runs]}), flush=True)
                return 0
            run = next((run for run in pending if not any(row["disposition"] == "valid completed" for row in run.filed.values())), pending[0])
            before = set(run.attempt_ids())
            note = "Subscription usage checked before dispatch; see the controller's saved meter receipt. Serial replacement queue."
            done = subprocess.run([sys.executable, str(queue.TOOLS / "run_cell.py"), "--run", str(run.dir),
                                   "--work", str(run.work), "--next", "--quota", note], cwd=queue.ROOT,
                                  env=env, capture_output=True, text=True)
            updated = queue.run_cell.Run(run.dir, run.work)
            attempts = set(updated.attempt_ids()) - before
            print(json.dumps({"run": run.id, "exit": done.returncode,
                              "output": (done.stdout + done.stderr)[-1500:], "status": queue.run_cell.status(updated)}), flush=True)
            if done.returncode or len(attempts) != 1:
                raise SystemExit("run_cell stopped; diagnose before dispatching again")
            record = updated.filed.get(attempts.pop())
            if record is None or record["disposition"] != "valid completed":
                raise SystemExit("review did not file validly; diagnose before dispatching again")
            ready([updated])
            if time.monotonic() >= next_meter:
                usage = meters(env)
                print(json.dumps({"meters": usage}), flush=True)
                next_meter = time.monotonic() + 3600


if __name__ == "__main__":
    main()
