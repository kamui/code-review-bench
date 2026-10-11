import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time


def now():
    return datetime.now(timezone.utc).isoformat()


def save(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def workload(root):
    command = [sys.executable, "-c", "import hashlib; print(hashlib.sha256(b'Z' * (32 * 1024 * 1024)).hexdigest())"]
    count = 0
    with (root / "workload.jsonl").open("x") as log:
        while not (root / "stop-workload").exists():
            began = time.monotonic()
            child = subprocess.Popen(command, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            output, error = child.communicate(timeout=30)
            count += 1
            row = {"iteration": count, "pid": child.pid, "started_monotonic": began,
                   "ended_monotonic": time.monotonic(), "exit_code": child.returncode,
                   "sha256": output.strip(), "stderr": error}
            log.write(json.dumps(row) + "\n")
            log.flush()
            save(root / "workload-progress.json", {"completed": count, "last": row})
            if child.returncode:
                return child.returncode
            time.sleep(0.1)
    return 0


def campaign(repo, root, client_bin):
    root.mkdir(exist_ok=True)
    if (root / "summary.json").exists():
        raise SystemExit("output already contains a campaign")
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
    state = subprocess.check_output(["git", "status", "--porcelain"], cwd=repo, text=True)
    if state:
        raise SystemExit("campaign requires a clean tree")
    env = {key: value for key, value in os.environ.items() if key != "BENCH_QUEUE_LAUNCH"}
    env["PATH"] = str(client_bin) + os.pathsep + env["PATH"]
    client = client_bin / "codex"
    summary = {"head": head, "tree_state": "clean", "host": platform.node(), "platform": platform.platform(),
               "uid": os.getuid(), "boot_id": Path("/proc/sys/kernel/random/boot_id").read_text().strip(),
               "client_version": subprocess.check_output([str(client), "--version"], env=env, text=True).strip(),
               "client_sha256": hashlib.sha256(client.read_bytes()).hexdigest(), "started_at": now(),
               "command": [sys.executable, "tools/run_bench_tests.py"], "runs": [],
               "workload": {"bytes_per_child": 32 * 1024 * 1024, "pause_seconds": 0.1,
                            "separate_session": True, "queue_marker": None}}
    work = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), "--workload", str(root)],
                            cwd=root, env=env, start_new_session=True)
    summary["workload"].update(pid=work.pid, session=os.getsid(work.pid))
    save(root / "summary.json", summary)
    try:
        deadline = time.monotonic() + 30
        while not (root / "workload-progress.json").exists():
            if work.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError("workload failed before the first suite")
            time.sleep(0.01)
        for index in range(1, 21):
            before = json.loads((root / "workload-progress.json").read_text())["completed"]
            began = time.monotonic()
            started_at = now()
            path = root / f"suite-{index:02d}.log"
            with path.open("x") as log:
                result = subprocess.run(summary["command"], cwd=repo, env=env, stdout=log, stderr=subprocess.STDOUT)
            ended = time.monotonic()
            progress = json.loads((root / "workload-progress.json").read_text())
            same_head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip() == head
            clean = not subprocess.check_output(["git", "status", "--porcelain"], cwd=repo, text=True)
            output = path.read_text(errors="replace")
            row = {"run": index, "started_at": started_at, "ended_at": now(), "started_monotonic": began,
                   "ended_monotonic": ended, "exit_code": result.returncode, "log": path.name,
                   "result": re.findall(r"Ran \d+ tests in [\d.]+s\n\n[^\n]+", output)[-1:],
                   "workload_completed_before": before, "workload_completed_after": progress["completed"],
                   "workload_alive": work.poll() is None, "same_head": same_head, "clean_tree": clean}
            summary["runs"].append(row)
            save(root / "summary.json", summary)
            print(json.dumps(row), flush=True)
            if result.returncode or not row["workload_alive"] or progress["completed"] <= before or not same_head or not clean:
                break
    finally:
        (root / "stop-workload").touch()
        summary["workload"]["exit_code"] = work.wait(timeout=35)
        summary["ended_at"] = now()
        summary["consecutive_passes"] = len(summary["runs"]) if all(
            row["exit_code"] == 0 and row["workload_alive"] and row["workload_completed_after"] > row["workload_completed_before"]
            and row["same_head"] and row["clean_tree"] for row in summary["runs"]) else 0
        save(root / "summary.json", summary)
    return 0 if summary["consecutive_passes"] == 20 and summary["workload"]["exit_code"] == 0 else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--workload", type=Path)
    parser.add_argument("--repo", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--client-bin", type=Path)
    args = parser.parse_args()
    raise SystemExit(workload(args.workload) if args.workload else campaign(args.repo, args.out, args.client_bin))
