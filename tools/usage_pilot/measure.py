#!/usr/bin/env python3
"""Measure the local usage pilot. Exit 0 on verified work, 1 on a failed trial.

Generate inputs with --prepare DIR; run with --inputs DIR --output NEW_DIR.
Inputs derive from the approved corpus. Results are never overwritten.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parents[2]
ROOT = Path(__file__).resolve().parent
BINARY = REPO / "rust/usage-pilot/target/release/usage-pilot"
CORPUS = REPO / "tools/behavior_contracts/usage.v1.json"
ENGINES = ("existing", "typed", "rust")
RATES = ["10", "50", "0.1", "1.25"]


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def generate(directory: Path) -> dict:
    directory.mkdir(parents=True, exist_ok=False)
    corpus = json.loads(CORPUS.read_text(encoding="utf-8"))
    case = next(c for c in corpus["cases"] if c["id"] == "duplicate-response-last")
    records = [json.loads(line) for line in base64.b64decode(case["initial"][0]["bytes_b64"]).splitlines()]
    inputs = {}
    for size, requests in (("small", 100), ("large", 100_000)):
        path = directory / f"{size}.jsonl"
        with path.open("xb") as handle:
            def emit(value: object) -> None:
                handle.write((json.dumps(value, separators=(",", ":")) + "\n").encode())
            emit({"type": "session_meta", "payload": {"id": "child", "parent_thread_id": "root"}})
            emit({"type": "turn_context", "payload": {"model": "pilot-model"}})
            for i in range(requests):
                if i % 4 == 0:
                    emit({"type": "response_item", "payload": {"type": "function_call", "arguments": "x" * 256}})
                if i % 10 == 0:
                    before = dict(records[0], payload=dict(records[0]["payload"], response_id=f"r-{i}"))
                    emit(before)
                final = dict(records[1], payload=dict(records[1]["payload"], response_id=f"r-{i}"))
                emit(final)
        expected = {
            "turns": requests, "tool_calls": requests // 4, "text_only_turns": requests * 3 // 4,
            "input": requests * 70, "cache_write": requests * 10, "cache_read": requests * 20,
            "output": requests * 9, "thinking": requests, "models": ["pilot-model"],
            "wall_seconds": 0,
            "cost": (requests * 70 * 10.0 + requests * 20 * 10.0 * 0.1
                     + requests * 10 * 10.0 * 1.25 + requests * 9 * 50.0) / 1_000_000,
        }
        data = path.read_bytes()
        inputs[size] = {"file": path.name, "requests": requests, "bytes": len(data),
                        "sha256": digest(data), "expected": expected}
    manifest = {"generator": "usage-pilot/1", "corpus_sha256": digest(CORPUS.read_bytes()), "inputs": inputs}
    (directory / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest


def command(engine: str, path: Path, mode: str) -> list[str]:
    if mode == "cli":
        if engine == "existing":
            return [sys.executable, str(REPO / "bench/tools/codex_usage.py"), str(path), "--prices", "10,50", "--json"]
        return [sys.executable, str(ROOT / "adapter.py"), "codex", "--engine", engine,
                str(path), "--prices", "10,50", "--json"]
    prefix = [str(BINARY)] if engine == "rust" else [sys.executable, str(ROOT / "python_core.py"), "--engine", engine]
    return prefix + [str(path)] + RATES + (["5", "1"] if mode == "warm" else ["1", "0"])


def capture(cmd: list[str]) -> str:
    result = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True, encoding="utf-8", check=False)
    return result.stdout.strip() if result.returncode == 0 else result.stderr.strip()


def environment() -> dict:
    sources = [p for base in (REPO / "rust/usage-pilot/src", ROOT) for p in base.glob("*.*") if p.suffix in (".py", ".rs")]
    sources += [REPO / "rust/usage-pilot/Cargo.lock", REPO / "rust/usage-pilot/Cargo.toml", BINARY]
    return {
        "started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "head": capture(["git", "rev-parse", "HEAD"]), "status": capture(["git", "status", "--short"]),
        "source_sha256": {str(p.relative_to(REPO)): digest(p.read_bytes()) for p in sources},
        "python": sys.version, "python_executable": sys.executable,
        "rustc": capture(["rustc", "--version", "--verbose"]), "cargo": capture(["cargo", "--version"]),
        "platform": platform.platform(), "cpu": capture(["lscpu"]), "cores": os.cpu_count(),
        "load": list(os.getloadavg()), "processes": capture(["ps", "-eo", "pid,comm,pcpu,pmem", "--sort=-pcpu"]),
        "limits": "Shared host; uncontrolled OS page cache; no external waits in measured commands. Peak RSS is the maximum process RSS including reaped descendants, not summed simultaneous tree RSS. Warm RSS includes warmup. CPU and elapsed for warm mode exclude warmup; process totals are also retained.",
    }


def run(inputs: Path, output: Path) -> int:
    manifest = json.loads((inputs / "manifest.json").read_text(encoding="utf-8"))
    for entry in manifest["inputs"].values():
        actual = (inputs / entry["file"]).read_bytes()
        if digest(actual) != entry["sha256"] or len(actual) != entry["bytes"]:
            raise ValueError("input identity changed")
    if sys.platform != "linux":
        raise ValueError("this measurement runner records Linux wait4 RSS in KiB")
    output.mkdir(parents=True, exist_ok=False)
    (output / "environment.json").write_text(json.dumps(environment(), indent=2) + "\n", encoding="utf-8")
    (output / "inputs.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    trials = []
    with (output / "trials.jsonl").open("x", encoding="utf-8") as raw:
        for size, entry in manifest["inputs"].items():
            path = (inputs / entry["file"]).resolve()
            for mode in ("cold", "warm", "cli"):
                for trial in range(7):
                    for engine in ENGINES[trial % 3:] + ENGINES[:trial % 3]:
                        cmd = command(engine, path, mode)
                        with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
                            start = time.perf_counter()
                            child = subprocess.Popen(cmd, cwd=REPO, stdout=stdout, stderr=stderr)
                            _, status, usage = os.wait4(child.pid, 0)
                            child.returncode = os.waitstatus_to_exitcode(status)
                            elapsed = time.perf_counter() - start
                            stdout.seek(0)
                            stderr.seek(0)
                            completed = subprocess.CompletedProcess(cmd, child.returncode, stdout.read(), stderr.read())
                        row = {"size": size, "mode": mode, "trial": trial + 1, "engine": engine,
                               "command": cmd, "exit": completed.returncode, "process_elapsed_seconds": elapsed,
                               "user_seconds": usage.ru_utime, "system_seconds": usage.ru_stime,
                               "peak_rss_kib": usage.ru_maxrss,
                               "resource_raw": dict(zip(("utime", "stime", "maxrss", "ixrss", "idrss", "isrss", "minflt", "majflt", "nswap", "inblock", "oublock", "msgsnd", "msgrcv", "nsignals", "nvcsw", "nivcsw"), usage)),
                               "stdout_sha256": digest(completed.stdout), "stderr": completed.stderr.decode("utf-8", "replace"),
                               "input_sha256": entry["sha256"], "load": list(os.getloadavg())}
                        try:
                            if completed.returncode != 0:
                                raise ValueError("nonzero exit")
                            repeats = 5 if mode == "warm" else 1
                            if mode == "cli":
                                summaries = [json.loads(completed.stdout)["total"]]
                                metadata = {"completed_iterations": 1, "bytes_processed": entry["bytes"]}
                            else:
                                summaries = [json.loads(line) for line in completed.stdout.splitlines()]
                                metadata = json.loads(completed.stderr)
                            if len(summaries) != repeats or any(s != entry["expected"] for s in summaries):
                                raise ValueError("independent expected summary mismatch")
                            if metadata["completed_iterations"] != repeats or metadata["bytes_processed"] != repeats * entry["bytes"]:
                                raise ValueError("completed-work count mismatch")
                            row.update({"verified": True, "summary": summaries[0], "completed_iterations": repeats,
                                        "bytes_processed": metadata["bytes_processed"], "loop": metadata,
                                        "elapsed_seconds": metadata["elapsed_seconds"] / repeats if mode == "warm" else elapsed,
                                        "cpu_seconds": metadata["cpu_seconds"] / repeats if mode == "warm" else usage.ru_utime + usage.ru_stime})
                        except (ValueError, KeyError, TypeError) as error:
                            row.update({"verified": False, "error": str(error), "stdout": completed.stdout.decode("utf-8", "replace")})
                        raw.write(json.dumps(row, separators=(",", ":")) + "\n")
                        raw.flush()
                        trials.append(row)
                        print(f"{size} {mode} {trial + 1} {engine}: {'ok' if row['verified'] else row['error']}", flush=True)
    summary = []
    for size in manifest["inputs"]:
        for mode in ("cold", "warm", "cli"):
            for engine in ENGINES:
                rows = [r for r in trials if (r["size"], r["mode"], r["engine"]) == (size, mode, engine)]
                result = {"size": size, "mode": mode, "engine": engine, "trials": len(rows), "failures": sum(not r["verified"] for r in rows)}
                if not result["failures"]:
                    for key in ("elapsed_seconds", "cpu_seconds", "peak_rss_kib"):
                        values = [r[key] for r in rows]
                        result[key] = {"median": statistics.median(values), "min": min(values), "max": max(values), "stdev": statistics.stdev(values)}
                summary.append(result)
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    return int(any(not r["verified"] for r in trials))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prepare", type=Path)
    parser.add_argument("--inputs", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.prepare:
        generate(args.prepare)
        return 0
    if args.inputs is None or args.output is None:
        parser.error("--inputs and --output are required")
    return run(args.inputs, args.output)


if __name__ == "__main__":
    sys.exit(main())
