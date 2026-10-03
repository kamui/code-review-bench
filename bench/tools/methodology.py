#!/usr/bin/env python3
"""Prepare a complete reconciliation queue without altering saved reviews or grades."""

import argparse
import json
from pathlib import Path

import current_grading

ROOT = Path(__file__).resolve().parents[2]


def plan(root=ROOT):
    selected, documents = current_grading.load_current(root)
    targets = [{"target": r["target"], "revision": r["revision"], "families": [f["id"] for f in r["families"]],
                "control": r["control"], "referenceAudit": "pending"}
               for r in documents["reference"]["targets"]]
    cells = {c["id"]: c for c in selected["cells"]}
    reviews = []
    for attempt in selected["attempts"]:
        if attempt["review"] is None:
            continue
        cell = cells[attempt["cell"]]
        document = current_grading.read_json(current_grading.resolve_pin(attempt["review"], root))
        reviews.append({"target": cell["target"], "run": cell["run"], "attempt": attempt["id"],
                        "review": attempt["review"], "record": attempt["record"], "items": len(document["items"]),
                        "parseStatus": document["parse_status"], "admission": attempt["admission"],
                        "action": "current-claim-assessment", "reason": "Selected scheduled cell and pinned source identity"})
    batches = [{**batch, "inputFingerprint": current_grading.grading_fingerprint(batch, selected, documents, documents["policy"], root)}
               for batch in selected["batches"]]
    return {"schemaVersion": 1, "contract": "current-reconciliation/v1", "counts": selected["counts"],
            "publication": "pending current assessments, reference calibration and evaluator audit",
            "targets": targets, "reviews": reviews, "batches": batches,
            "sharedClaimItems": [{"claim": c["id"], "target": c["target"], "links": c["links"], "adjudication": c["adjudication"]}
                                 for c in documents["claim"]["claims"]]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    result = plan()
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("x", encoding="utf-8") as handle:
        handle.write(json.dumps(result, indent=2) + "\n")
    print(f"Saved {len(result['reviews'])} selected reviews, {len(result['batches'])} batches, "
          f"{len(result['sharedClaimItems'])} shared-claim links. No grades changed.")


if __name__ == "__main__":
    main()
