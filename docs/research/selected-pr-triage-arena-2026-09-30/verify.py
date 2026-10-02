import hashlib
import json
import random
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent


def read(path):
    return json.loads(path.read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


receipt = read(HERE / "artifact-receipt.v1.json")
for entry in receipt["files"]:
    assert digest(ROOT / entry["path"]) == entry["sha256"], entry["path"]

protected = read(HERE / "protected-inputs.v1.json")
for entry in protected["files"]:
    assert digest(ROOT / entry["path"]) == entry["sha256"], entry["path"]

grounding = read(HERE / "grounding.json")
assert digest(HERE / "grounding.json") == read(HERE / "grounding-manifest.json")["sha256"]
inventory = read(ROOT / "docs/research/selected-pr-adjudication-2026-09-30/inventory.v1.json")
random.Random(691916631).shuffle(inventory)
assert len(inventory) == len(grounding["items"]) == 68
for number, (original, blinded) in enumerate(zip(inventory, grounding["items"]), 1):
    assert blinded == {"token": f"R{number:03}", "target": original["target"], "item": original["item"]}
    assert digest(ROOT / original["review"]["path"]) == original["review"]["sha256"]

originals = {entry["token"]: entry for entry in grounding["items"]}
for name in ("candidates/sol/audit.json", "candidates/opus/audit.json", "triage.v1.json"):
    audit = read(HERE / name)
    rows = audit["items"]
    assert len(rows) == len({row["token"] for row in rows}) == 68
    assert {row["token"] for row in rows} == set(originals)
    canonical = {entry["id"]: entry for entry in audit["canonical_issues"]}
    assert len(canonical) == len(audit["canonical_issues"])
    for row in rows:
        assert row["target"] == originals[row["token"]]["target"]
        if "original_item" in row:
            assert row["original_item"] == originals[row["token"]]["item"]
        else:
            item = originals[row["token"]]["item"]
            assert (row["claim"], row["file"], row["lines"]) == (item["claim"], item["file"], [item["line_start"], item["line_end"]])
        assert row["canonical_issue_ids"]
        for identity in row["canonical_issue_ids"]:
            assert row["token"] in canonical[identity]["original_tokens"]
    for identity, entry in canonical.items():
        linked = {row["token"] for row in rows if identity in row["canonical_issue_ids"]}
        assert linked == set(entry["original_tokens"])

triage = read(HERE / "triage.v1.json")
canonical = {entry["id"]: entry for entry in triage["canonical_issues"]}
for row in triage["items"]:
    assert row["decision"] is None and row["administrative_approval_needed"] is True
    proposals = [canonical[identity] for identity in row["canonical_issue_ids"]]
    assert row["proposed_outcomes"] == [entry["proposed_outcome"] for entry in proposals]
    assert (row["route"] == "needs-human") == any(entry["route"] == "needs-human" for entry in proposals)
    assert bool(row["human_question"]) == (row["route"] == "needs-human")
for entry in triage["canonical_issues"]:
    assert entry["decision"] is None
    assert entry["administrative_approval_needed"] is True
    for path in entry["evidence_paths"]:
        assert (ROOT / path).is_file(), path
for entry in triage["research_only"]:
    assert entry["original_tokens"] == []
    for path in entry["evidence_paths"]:
        assert (ROOT / path).is_file(), path
assert len(triage["research_only"]) == 4
assert triage["administrative_approval"]["required"] is True
queue = triage["human_queue"]
assert len(queue) == 1 and queue[0]["decision"] is None
assert set(queue[0]["original_tokens"]) == {row["token"] for row in triage["items"] if row["route"] == "needs-human"}
assert queue[0]["canonical_issue_ids"] == [entry["id"] for entry in triage["canonical_issues"] if entry["route"] == "needs-human"]
assert triage["coverage"]["item_routes"] == dict(Counter(row["route"] for row in triage["items"]))
assert triage["coverage"]["canonical_outcomes"] == dict(Counter(entry["proposed_outcome"] for entry in triage["canonical_issues"]))
assert triage["evidence_queue"] == []

verification = {
    "schema_version": 1,
    "result": "pass",
    "checks": [
        "All 31 raw candidate, judge, grounding, prompt and additional source artifacts match saved hashes.",
        "Protected policy, registry and prior intake inputs match saved hashes.",
        "The deterministic blinded shuffle reproduces all 68 exact original items from hashed saved reviews.",
        "Both candidate audits and the synthesis cover each token exactly once with complete reciprocal canonical membership.",
        "Synthesized originals, proposed outcomes, pending decisions, evidence paths and routes are consistent.",
        "The one human question exactly covers all nine needs-human items.",
        "Four research hypotheses remain outside the original denominator.",
    ],
    "coverage": triage["coverage"],
    "limits": "Structural and provenance validation does not independently prove technical eligibility or grant human authority.",
}
(HERE / "verification.v1.json").write_text(json.dumps(verification, indent=2) + "\n")
print(json.dumps({"result": "pass", "coverage": triage["coverage"], "artifact_hashes": len(receipt["files"]), "protected_hashes": len(protected["files"])}, indent=2))
