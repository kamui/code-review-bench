#!/usr/bin/env python3
"""Write the after-record of each decision of the activation round that `after.v1.json` holds.

    python3 docs/research/regrade-v3-rulings-2026-10-23/after.py

Run from the repository root. `after.v1.json` holds what the owner decided for a before-record, taken from the
owner's log, `owner-answers.md` beside this script: the outcome, how many times the owner was asked, the owner's own words where the log
gives a reason, and what the first answers missed. A record is written only when `bench-tools ruling check`
accepts it beside its before-record. Running it again changes nothing. It closes the round when every before-record has its after-record."""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
RULINGS = HERE / "rulings"
OPTIONAL = ("same_fault_as", "facts")

if __name__ == "__main__":
    decided = json.loads((HERE / "after.v1.json").read_text(encoding="utf-8"))["records"]
    for name, entry in decided.items():
        before = RULINGS / f"{name}.before.json"
        record = {"contract": 2, "ruling": json.loads(before.read_text(encoding="utf-8"))["ruling"],
                  "before": {"path": before.relative_to(ROOT).as_posix(), "sha256": hashlib.sha256(before.read_bytes()).hexdigest()},
                  "settled_by": "owner", "policy": None, "asked": entry["asked"], "outcome": entry["outcome"], "by_default": False,
                  "ground": entry["ground"], "miss": entry["miss"], "lesson": entry["lesson"],
                  **{field: entry[field] for field in OPTIONAL if field in entry}}
        after = RULINGS / f"{name}.after.json"
        text = json.dumps(record, indent=1, ensure_ascii=False) + "\n"
        kept = after.read_text(encoding="utf-8") if after.exists() else None
        after.write_text(text, encoding="utf-8")
        checked = subprocess.run(["bench/tools/bench-tools", "ruling", "check", before.relative_to(ROOT).as_posix(), after.relative_to(ROOT).as_posix()], cwd=ROOT, capture_output=True, text=True)
        if checked.returncode:
            after.write_text(kept, encoding="utf-8") if kept is not None else after.unlink()
            raise SystemExit(f"{name}: {checked.stdout}{checked.stderr}")
        print(name, entry["outcome"])
    befores = {path.name.removesuffix(".before.json") for path in RULINGS.glob("*.before.json")}
    if befores == set(decided):
        round_file = RULINGS / "round.json"
        state = json.loads(round_file.read_text(encoding="utf-8"))
        if not state["closed"]:
            state["closed"] = True
            round_file.write_text(json.dumps(state, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
        print("every before-record has its after-record; the round is closed")
