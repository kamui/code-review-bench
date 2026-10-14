#!/usr/bin/env python3
"""Add the amendment of 2026-10-13 to the audit declaration in `bench/grading/current/audits.json`.

    python3 docs/research/older-problem-activation-2026-10-13/filing/audit.py

Run from the repository root, after `receipts.py` has written the fourth audit receipt. The plan gains the three
strata for pre-existing problems, their sizes and the one tolerance, with the fourth whole-cohort check and the rule
for the draw in its `selection` text, as the synthesized proposal the owner chose says. The thirteen strata in force,
their sizes, their tolerances and the seed are not changed. Running it again changes nothing."""
import hashlib
import json
from pathlib import Path

import receipts

ROOT = Path(__file__).resolve().parents[4]
AUDITS = ROOT / "bench/grading/current/audits.json"
RECEIPT = "bench/grading/rulings/cohort-rebuild-audit.v4.md"
UNIT = ("One admitted review and one admitted pre-existing problem of its task. Its result is raised when a claim of the review has says_what "
        "\"yes\" for the problem, otherwise unresolved when one has \"cannot-tell\", otherwise not-raised. ")
STRATA = [
    {"id": "pre-existing-raised", "unit": "pre-existing-review", "population": UNIT + "This stratum holds the units whose result is raised."},
    {"id": "pre-existing-unresolved", "unit": "pre-existing-review", "population": UNIT + "This stratum holds the units whose result is unresolved."},
    {"id": "pre-existing-not-raised", "unit": "pre-existing-review", "population": UNIT + "This stratum holds the units whose result is not-raised."}]
SIZES = {"pre-existing-raised": "all", "pre-existing-unresolved": "all", "pre-existing-not-raised": 60}
LIMITS = {"pre-existing-raised": 0, "pre-existing-unresolved": 0, "pre-existing-not-raised": 1}
BASIS = (" Amended on 2026-10-13 by audit decision 8, in the words of the proposal the owner chose: \"This amendment is declared before any grade "
         "under contract v3 exists, but not blind: a first regrade under contract v2 is saved, its aggregate outcomes are known (53 claims labelled "
         "outside this change), and the proposer knows that comments from 16 reviews were linked to the first five candidates (reported).\"")
SELECTION = (" A pre-existing-review unit id is '<run>/<attempt>#pre-existing:<problem>'. A stratum whose size is \"all\" is always audited whole. "
             "A fourth check covers the whole cohort before the draw, with no sample: for every admitted review and every pre-existing problem of "
             "its task there is exactly one saved result, and it equals the derivation in the stratum's population; no claim names both a known "
             "problem and a pre-existing problem; and no result of raised rests on a candidate that is still pending or on a claim that is neither "
             "outside-this-change nor unresolved for want of evidence. The sample is drawn only when no candidate for a pre-existing problem is "
             "pending: each candidate the regrade raises is ruled on first, the batches of a task with a new admission are graded again, and the "
             "draw follows.")

if __name__ == "__main__":
    audits = json.loads(AUDITS.read_text(encoding="utf-8"))
    plan = audits["plan"]
    pin = {"path": RECEIPT, "sha256": hashlib.sha256((ROOT / RECEIPT).read_bytes()).hexdigest()}
    names = [stratum["id"] for stratum in plan["strata"]]
    plan["strata"] += [stratum for stratum in STRATA if stratum["id"] not in names]
    for text, field in ((BASIS, "basis"), (SELECTION, "selection")):
        if text not in plan[field]:
            plan[field] += text
    sample, tolerances = plan["sample"], plan["tolerances"]
    sample["sizes"] += [{"stratum": name, "size": size} for name, size in SIZES.items() if name not in [entry["stratum"] for entry in sample["sizes"]]]
    tolerances["limits"] += [{"stratum": name, "max_errors": limit} for name, limit in LIMITS.items()
                             if name not in [entry["stratum"] for entry in tolerances["limits"]]]
    sample.update({"receipt": pin, "receipt_scope": receipts.AMENDED_SAMPLE,
                   "reason": "The user selected 100 units in the serious stratum, 60 in six strata, 40 in three and 30 in four, and every unit of the "
                             "two strata for pre-existing problems raised and unresolved."})
    tolerances.update({"receipt": pin, "receipt_scope": receipts.AMENDED_TOLERANCE,
                       "reason": "The user selected at most 0 confirmed first-grader errors in the unresolved-credit stratum, at most 2 in suggestion "
                                 "and at most 1 in each other sampled stratum. The two strata audited whole have no tolerance; the 0 entered for each "
                                 "is required by the file's form and is not used."})
    AUDITS.write_text(json.dumps(audits, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print("declared", len(plan["strata"]), "strata")
