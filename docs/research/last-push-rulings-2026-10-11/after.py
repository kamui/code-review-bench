#!/usr/bin/env python3
"""Write the after-record of one decision the user settled.

    python3 docs/research/last-push-rulings-2026-10-11/after.py BEFORE.json OUTCOME [--unneeded-label] [--asked N]

--unneeded-label is for a label prepared in case a claim was ruled a problem, when the user ruled it was not:
the outcome is `not-applicable`, and the record says no label was decided."""
import argparse
import hashlib
import json
from pathlib import Path

UNNEEDED = ("No label was decided: the user ruled the case off the answer key, so the outcome is not-applicable. The recommender's and the inspector's labels "
            "were prepared in case it was ruled a problem. The list of causes has no entry for a label that was never needed, and the nearest one is entered.")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("before")
    parser.add_argument("outcome")
    parser.add_argument("--unneeded-label", action="store_true")
    parser.add_argument("--asked", type=int, default=1)
    args = parser.parse_args()
    before = Path(args.before)
    record = json.loads(before.read_text(encoding="utf-8"))
    after = {"contract": 2, "ruling": record["ruling"], "before": {"path": str(before), "sha256": hashlib.sha256(before.read_bytes()).hexdigest()},
             "settled_by": "owner", "policy": None, "asked": args.asked, "outcome": args.outcome, "by_default": False, "ground": None,
             "miss": {"cause": "owner-weighs-differently", "note": UNNEEDED} if args.unneeded_label else None, "lesson": None}
    out = before.with_name(before.name.replace(".before.json", ".after.json"))
    out.write_text(json.dumps(after, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    print(out)


if __name__ == "__main__":
    main()
