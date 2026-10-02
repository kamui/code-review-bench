#!/usr/bin/env python3
"""Freeze the 2026-10-02 skill-matrix runs at the commit that holds their definitions.

Usage::

    python3 docs/research/skill-matrix-2026-10-02/freeze.py <commit>

The commit must contain every run's manifest, inputs and arm files exactly as they are on disk.
An already frozen manifest is refused.
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
            raise SystemExit(f"{path} is already frozen")
        relative = path.relative_to(ROOT).as_posix()
        committed = subprocess.check_output(["git", "-C", str(ROOT), "show", f"{commit}:{relative}"])
        if committed != path.read_bytes():
            raise SystemExit(f"{relative} differs from {commit}")
        for arm in manifest["arms"]:
            arm_path = f"bench/arms/{arm['id']}.json"
            if subprocess.check_output(["git", "-C", str(ROOT), "show", f"{commit}:{arm_path}"]) != (ROOT / arm_path).read_bytes():
                raise SystemExit(f"{arm_path} differs from {commit}")
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
