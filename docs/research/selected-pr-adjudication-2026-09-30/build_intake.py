#!/usr/bin/env python3
"""Build exclusive adjudication records from inspected claims and saved evidence."""
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / "bench/tools"))
import claims


def ref(path):
    return claims.reference(ROOT / path)


def dump(path, value):
    with path.open("x") as stream:
        stream.write(json.dumps(value, indent=2) + "\n")


def main():
    definitions = json.loads((OUT / "assessments.v1.json").read_text())
    inventory = json.loads((OUT / "inventory.v1.json").read_text())
    cases = []
    assigned = set()
    for row in definitions:
        matching = []
        for item in inventory:
            if item["target"] != row["target"]:
                continue
            key = item["attempt_id"] + ":" + item["item_id"]
            if key not in row.get("equivalent", []) + row.get("related", []):
                continue
            relation = "equivalent" if key in row.get("equivalent", []) else "related"
            if relation == "equivalent":
                assert key not in assigned
                assigned.add(key)
            matching.append({k: item[k] for k in ("mapping", "review", "attempt_id", "item_id")} | {
                "relation": relation,
                "reason": row["match_reason"] if relation == "equivalent" else row["related_reason"],
            })
        row["links"] = matching
        if not matching or row.get("component_only"):
            continue
        target = json.loads((ROOT / "bench/targets" / row["target"] / "target.json").read_text())
        case = {
            "schema_version": 1, "claim_id": row["claim_id"], "version": 1,
            "target": row["target"],
            "revision": {k: target[k] for k in ("head", "base_sha", "packet_sha256", "diff_manifest_sha256")},
            "supersedes": None,
            "revision_reason": "Initial inspected intake from saved reviews. Recommendation is separate from eligibility authority; no human ruling or grading is inferred.",
            "claim": {k: row[k] for k in ("trigger", "mechanism", "consequence", "change_relation", "settlement_question")},
            "links": matching,
            "evidence": [{"source": ref(e["path"]), "stance": e["stance"], "summary": e["summary"]} for e in row["evidence"]],
            "decision": None,
        }
        path = ROOT / "bench/claims" / (row["claim_id"] + ".v1.json")
        dump(path, case)
        cases.append(ref(path))
    assert len(assigned) == len(inventory) - 1, (len(assigned), len(inventory))
    assert {i["attempt_id"] + ":" + i["item_id"] for i in inventory} - assigned == {"att-007:item-0"}
    registry = json.loads((ROOT / "bench/claims/registry.json").read_text())
    dump(ROOT / "bench/claims/registry.selected-pr-intake-v1.json", {"schema_version": 1, "cases": registry["cases"] + cases})
    dump(OUT / "queue.v1.json", {
        "schema_version": 1, "status": "awaiting human rulings", "grading_performed": False,
        "source_run": "2026-09-30-selected-prs-review-only",
        "inventory": ref(OUT / "inventory.v1.json"),
        "assessment_source": ref(OUT / "assessments.v1.json"),
        "registry": ref(ROOT / "bench/claims/registry.selected-pr-intake-v1.json"),
        "review_count": 45, "item_count": len(inventory),
        "linked_case_count": len(cases), "questions": definitions,
    })
    _, loaded = claims.load_registry(ROOT / "bench/claims/registry.selected-pr-intake-v1.json")
    text, key = claims.dossier([c for c in loaded if c["claim_id"] in {r["claim_id"] for r in definitions}])
    with (OUT / "dossier.v1.md").open("x") as stream:
        stream.write(text)
    private = ROOT / ".local/selected-pr-adjudication"
    private.mkdir(parents=True, exist_ok=True)
    dump(private / "dossier-key.v1.json", key)
    print(f"{len(cases)} pending canonical cases; {len(inventory)} original items; {len(definitions)} settlement questions")


if __name__ == "__main__":
    main()
