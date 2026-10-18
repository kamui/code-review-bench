#!/usr/bin/env python3
"""Run isolated pilot gates and preserve each check's output and elapsed time.

Usage: python3 tools/usage_pilot/verify.py --output NEW_DIRECTORY
Exit 0 when all checks pass, 1 on a failed check. No network or model calls.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parents[2]
RUST = REPO / "rust/usage-pilot"
ROOT = Path(__file__).resolve().parent


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    sources = [p for base in (RUST / "src", RUST / "tests", RUST / "examples", ROOT) for p in base.glob("*.*") if p.suffix in (".py", ".rs")]
    context = {"head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip(),
               "source_sha256": {str(p.relative_to(REPO)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sources}}
    results = []
    with tempfile.TemporaryDirectory(prefix="usage-pilot-checks-") as tmp:
        maps = {}
        for engine in ("typed", "rust"):
            path = Path(tmp) / f"{engine}.json"
            path.write_text(json.dumps({"codex": [sys.executable, str(ROOT / "adapter.py"), "codex", "--engine", engine],
                                       "transcript": [sys.executable, str(ROOT / "adapter.py"), "transcript"]}), encoding="utf-8")
            maps[engine] = str(path)
        checks = [
            ("format", RUST, ["cargo", "fmt", "--check"], {}),
            ("clippy", RUST, ["cargo", "clippy", "--locked", "--offline", "--all-targets", "--", "-D", "warnings"], {}),
            ("rust-dev", RUST, ["cargo", "test", "--locked", "--offline"], {}),
            ("rust-release", RUST, ["cargo", "test", "--locked", "--offline", "--release"], {}),
            ("release-binary", RUST, ["cargo", "build", "--locked", "--offline", "--release", "--examples", "--bins"], {}),
            ("typed-python", REPO, [sys.executable, "-m", "unittest", "discover", "-s", str(ROOT), "-p", "test_typed_usage.py", "-v"], {}),
            ("properties-dev", REPO, [sys.executable, "-m", "unittest", "discover", "-s", str(ROOT), "-p", "test_differential.py", "-v"], {"USAGE_PILOT_BINARY": str(RUST / "target/debug/usage-pilot")}),
            ("properties-release", REPO, [sys.executable, "-m", "unittest", "discover", "-s", str(ROOT), "-p", "test_differential.py", "-v"], {}),
            ("baseline-corpus", REPO, [sys.executable, "tools/behavior_contract.py", "--allow-known-defects"], {}),
            ("typed-corpus", REPO, [sys.executable, "tools/behavior_contract.py", "--implementations", maps["typed"]], {}),
            ("rust-corpus", REPO, [sys.executable, "tools/behavior_contract.py", "--implementations", maps["rust"]], {}),
            ("typed-differential", REPO, [sys.executable, "tools/behavior_contract.py", "--allow-known-defects", "--differential", maps["typed"]], {}),
            ("rust-differential", REPO, [sys.executable, "tools/behavior_contract.py", "--allow-known-defects", "--differential", maps["rust"]], {}),
            ("usage-regressions", REPO, [sys.executable, "-m", "unittest", "discover", "-s", "bench/tools", "-p", "test_*usage.py", "-v"], {}),
            ("codex-self-test", REPO, [sys.executable, "bench/tools/codex_usage.py", "--self-test"], {}),
            ("transcript-self-test", REPO, [sys.executable, "bench/tools/transcript_usage.py", "--self-test"], {}),
            ("contract-tests", REPO, [sys.executable, "-m", "unittest", "discover", "-s", "tools", "-p", "test_behavior_contract.py", "-v"], {}),
            ("decision-history", REPO, [sys.executable, "tools/behavior_contract.py", "--verify-decision-history"], {}),
        ]
        for name, cwd, command, env in checks:
            start = time.perf_counter()
            result = subprocess.run(command, cwd=cwd, env=dict(os.environ, **env), capture_output=True, check=False)
            elapsed = time.perf_counter() - start
            (args.output / f"{name}.log").write_bytes(result.stdout + result.stderr)
            results.append({"check": name, "command": command, "cwd": str(cwd), "environment": env,
                            "exit": result.returncode, "elapsed_seconds": elapsed, "log": f"{name}.log"})
            print(f"{name}: exit {result.returncode}, {elapsed:.3f}s", flush=True)
            (args.output / "checks.json").write_text(json.dumps(dict(context, checks=results), indent=2) + "\n", encoding="utf-8")
    return int(any(row["exit"] != 0 for row in results))


if __name__ == "__main__":
    sys.exit(main())
