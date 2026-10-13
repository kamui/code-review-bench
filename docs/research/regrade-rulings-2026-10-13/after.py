#!/usr/bin/env python3
"""Write the after-record of every decision of the regrade round and close the round.

    python3 docs/research/regrade-rulings-2026-10-13/after.py

Run from the repository root. `after.v1.json` holds what the owner decided for each before-record, taken from the
"Answer" section of its ruling file. Every before-record needs an entry and every entry a before-record. Each
record is checked with `bench/tools/ruling_record.py` before anything is written, and the files change together:
an error leaves the directory as it was. Running it again changes nothing.

A label that was prepared and never decided is recorded as `not-applicable` with the nearest cause, as the last
round recorded one, because the lists of outcomes and causes have no entry for it."""
import hashlib
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(ROOT / "bench/tools"))
sys.path.insert(0, str(HERE / "filing"))
import ruling_record  # noqa: E402
from record import Generation  # noqa: E402

RULINGS = HERE / "rulings"
UNNEEDED = {
    "off": "No label was decided: the user ruled the case off the answer key, so the outcome is not-applicable. The recommender's and the "
           "inspector's labels were prepared in case it was ruled a problem. The list of causes has no entry for a label that was never "
           "needed, and the nearest one is entered.",
    "joined": "No label of its own was decided: the user ruled the case part of {family}, which keeps its label, {band}. The recommender's "
              "and the inspector's labels were prepared in case it was kept apart. The list of outcomes has no entry for a label that was "
              "never needed because the case joined a known problem, and the list of causes has none either; not-applicable and the "
              "nearest cause are entered."}
OPTIONAL = ("same_fault_as", "facts")


def after(before, decided):
    record = {"contract": 2, "ruling": json.loads(before.read_text(encoding="utf-8"))["ruling"],
              "before": {"path": before.relative_to(ROOT).as_posix(), "sha256": hashlib.sha256(before.read_bytes()).hexdigest()},
              "settled_by": "owner", "policy": None, "asked": decided["asked"], "outcome": decided["outcome"], "by_default": False,
              "ground": decided["ground"], "miss": decided["miss"], "lesson": decided["lesson"]}
    if "unneeded" in decided:
        record["miss"] = {"cause": "owner-weighs-differently", "note": UNNEEDED[decided["unneeded"]].format(**decided) + decided.get("more", "")}
    return {**record, **{field: decided[field] for field in OPTIONAL if field in decided}}


def main():
    decisions = json.loads((HERE / "after.v1.json").read_text(encoding="utf-8"))["records"]
    befores = {path.name.removesuffix(".before.json"): path for path in sorted(RULINGS.glob("*.before.json"))}
    if set(befores) != set(decisions):
        sys.exit(f"after.v1.json and the before-records differ: {sorted(set(befores) ^ set(decisions))}")
    sheet = json.loads((RULINGS / "round.json").read_text(encoding="utf-8"))
    with Generation() as files:
        for stem, before in befores.items():
            record = after(before, decisions[stem])
            files.write(before.with_name(f"{stem}.after.json"), json.dumps(record, indent=1, ensure_ascii=False) + "\n")
            faults = ruling_record.after_faults(json.loads(before.read_text(encoding="utf-8")), record, before)
            if faults:
                raise SystemExit(f"{stem}: " + "; ".join(faults))
        files.write(RULINGS / "round.json", json.dumps({**sheet, "closed": True}, indent=1) + "\n")
        faults = ruling_record.round_faults(RULINGS)
        if faults:
            raise SystemExit("; ".join(faults))
    print(f"wrote {len(befores)} after-records and closed the round")


if __name__ == "__main__":
    main()
