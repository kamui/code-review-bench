#!/usr/bin/env python3
"""Fetch the freeze commit of every run that pins re-cut packets and is missing from this clone.

    python3 tools/fetch_freeze_commits.py

Packet selection reads such a run's manifest at its ``freeze_commit``. A squash merge leaves that commit
off ``main``, and a shallow checkout lacks it either way. The forge still serves it by its id, through
the pull request that carried it. Exit 0 when every such commit is present afterwards, 1 otherwise."""
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def present(commit: str) -> bool:
    return subprocess.run(["git", "-C", str(ROOT), "cat-file", "-e", commit + "^{commit}"], capture_output=True).returncode == 0


def main() -> int:
    commits = set()
    for path in sorted((ROOT / "bench/runs").glob("*/manifest.json")):
        manifest = json.loads(path.read_text(encoding="utf-8"))
        if "packet_replacements" in manifest and manifest.get("freeze_commit"):
            commits.add(manifest["freeze_commit"])
    missing = sorted(commit for commit in commits if not present(commit))
    if missing:
        shallow = subprocess.check_output(
            ["git", "-C", str(ROOT), "rev-parse", "--is-shallow-repository"], text=True).strip() == "true"
        depth = ["--depth=1"] if shallow else []
        subprocess.run(["git", "-C", str(ROOT), "fetch", "--no-tags", *depth, "origin", *missing], check=False)
    absent = [commit for commit in missing if not present(commit)]
    for commit in absent:
        print(f"freeze commit {commit} could not be fetched", file=sys.stderr)
    print(f"{len(commits)} freeze commits, {len(missing) - len(absent)} fetched, {len(absent)} absent")
    return 1 if absent else 0


if __name__ == "__main__":
    sys.exit(main())
