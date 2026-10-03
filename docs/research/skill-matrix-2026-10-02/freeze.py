#!/usr/bin/env python3
"""Freeze the 2026-10-02 skill-matrix runs at the commit that holds their definitions.

Usage::

    python3 docs/research/skill-matrix-2026-10-02/freeze.py <commit>

It freezes every unfrozen 2026-10-02 run. The commit must contain each such run's manifest, inputs
and arm files exactly as they are on disk.
"""

import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def main():
    commit = subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", sys.argv[1] + "^{commit}"], text=True).strip()
    now = datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")
    for path in sorted((ROOT / "bench/runs").glob("2026-10-02-*/manifest.json")):
        manifest = json.loads(path.read_text(encoding="utf-8"))
        if manifest.get("frozen_at"):
            continue
        inputs = path.parent / "inputs"
        on_disk = sorted(file.relative_to(ROOT).as_posix() for file in inputs.rglob("*") if not file.is_dir())
        listed = subprocess.check_output(["git", "-C", str(ROOT), "ls-tree", "-r", "-z", "--name-only", commit, "--",
                                          inputs.relative_to(ROOT).as_posix()], text=True)
        if on_disk != sorted(filter(None, listed.split("\0"))):
            raise SystemExit(f"{inputs.relative_to(ROOT).as_posix()} does not hold the files of {commit}")
        arms = [f"bench/arms/{arm['id']}.json" for arm in manifest["arms"]]
        for relative in [path.relative_to(ROOT).as_posix(), *on_disk, *arms]:
            if subprocess.check_output(["git", "-C", str(ROOT), "show", f"{commit}:{relative}"]) != (ROOT / relative).read_bytes():
                raise SystemExit(f"{relative} differs from {commit}")
        manifest = {key: value for key, value in manifest.items() if key != "metric_code_revision"}
        ordered = {}
        for key, value in manifest.items():
            ordered[key] = value
            if key == "created_at":
                ordered.update(frozen_at=now, freeze_commit=commit)
            if key == "rubric_version":
                ordered["metric_code_revision"] = commit
        path.write_text(json.dumps(ordered, indent=2) + "\n", encoding="utf-8")
        print("frozen", path.parent.name)


if __name__ == "__main__":
    main()
