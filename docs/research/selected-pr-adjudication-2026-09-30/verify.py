#!/usr/bin/env python3
"""Verify the saved intake without grading claims or modifying evidence."""
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / "bench/tools"))
import claims


def read(path):
    return json.loads(path.read_text())


def check_ref(ref):
    return claims.resolve(ref)


def main():
    queue = read(OUT / "queue.v1.json")
    for name in ("inventory", "assessment_source", "registry"):
        check_ref(queue[name])
    inventory = read(check_ref(queue["inventory"]))
    _, cases = claims.load_registry(check_ref(queue["registry"]))
    new_cases = [case for case in cases if case["claim_id"] in {
        row["claim_id"] for row in queue["questions"]
    }]
    assert len(new_cases) == 14
    assert all(case["decision"] is None for case in new_cases)
    assert queue["grading_performed"] is False
    assert len(queue["questions"]) == 19
    keys = set()
    for row in inventory:
        check_ref(row["review"])
        claims.source_item(row, row["target"])
        key = (row["review"]["path"], row["item_id"])
        assert key not in keys
        keys.add(key)
    linked = {(link["review"]["path"], link["item_id"])
              for case in new_cases for link in case["links"]}
    assert linked == keys
    assert len(keys) == 68
    equivalent = Counter((link["review"]["path"], link["item_id"])
                         for case in new_cases for link in case["links"]
                         if link["relation"] == "equivalent")
    assert len(equivalent) == 67 and set(equivalent.values()) == {1}
    run = ROOT / "bench/runs" / queue["source_run"]
    reviews = read(run / "review-inventory.json")
    assert len(reviews) == queue["review_count"] == 45
    counts = Counter()
    for review in reviews:
        counts[review["cell"]["target"]] += 1
        normalized = read(run / "attempts" / review["attempt"] / "normalized.json")
        assert len(normalized["items"]) == review["finding_count"]
    assert set(counts.values()) == {9}
    for row in queue["questions"]:
        assert row["recommendation"] and row["reasoning"] and row["limits"]
        for entry in row["evidence"]:
            assert (ROOT / entry["path"]).is_file()
    for name in ("source-manifest.v1.json", "probe-receipt.v1.json"):
        doc = read(OUT / "evidence" / name)
        for ref in doc if isinstance(doc, list) else doc["probe_files"]:
            check_ref({key: ref[key] for key in ("path", "sha256")})
    upstream = read(OUT / "evidence/upstream/manifest.v1.json")
    with tarfile.open(ROOT / upstream[0]["archive"]) as archive:
        for entry in upstream:
            path = check_ref({key: entry[key] for key in ("path", "sha256")})
            assert path.read_bytes() == archive.extractfile(entry["archive_member"]).read()
    for name in ("review.v1.md", "dossier.v1.md"):
        text = (OUT / name).read_text()
        for target in re.findall(r"\[[^\]]*\]\(([^)]+)\)", text):
            if not target.startswith(("https://", "http://", "#")):
                assert (OUT / target.split("#", 1)[0]).exists(), target
    print("45 reviews; 68 original items; 14 pending canonical cases; 19 questions; all source hashes and links verified")


if __name__ == "__main__":
    main()
