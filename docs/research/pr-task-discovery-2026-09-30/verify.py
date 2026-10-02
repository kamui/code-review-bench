#!/usr/bin/env python3
"""Verify shortlist pins, quotas, frozen portfolios and preserved source bytes."""

import hashlib
import json
from pathlib import Path
import re
import tarfile


ROOT = Path(__file__).resolve().parent


def read_json(name):
    return json.loads((ROOT / name).read_text())


def verify():
    recommendation = read_json("recommendations.json")
    rows = recommendation["ranked_candidates"]
    assert len(rows) == 15
    assert len({(row["repository"], row["pr_number"]) for row in rows}) == 15
    for bucket in "ABCDE":
        assert sorted(row["rank"] for row in rows if row["bucket"] == bucket) == [1, 2, 3]
    assert {row["subtype"] for row in rows if row["bucket"] == "D"} == {
        "architecture", "maintainability", "scalability"
    }
    assert all(row["human_eligibility_ruling"] is None for row in rows)
    assert all(
        row["whole_pr_clean_status"] == "unresolved"
        for row in rows if row["bucket"] == "E"
    )
    comparisons = read_json("revision-verification.json")
    assert len(comparisons) == 30
    assert all(row["pins_verified"] for row in comparisons)
    assert all(row["true_pr_merge_base_verified"] for row in comparisons)
    checked = {
        (row["repo"], row["pr"], row["base"], row["head"])
        for row in comparisons
    }
    for row in rows:
        assert re.fullmatch(r"[0-9a-f]{40}", row["base_sha"])
        assert re.fullmatch(r"[0-9a-f]{40}", row["review_head_sha"])
        assert (
            row["repository"], row["pr_number"],
            row["base_sha"], row["review_head_sha"]
        ) in checked
    for record in read_json("frozen-portfolios.json") + read_json("frozen-debates.json"):
        assert hashlib.sha256((ROOT / record["path"]).read_bytes()).hexdigest() == record["sha256"]
    for candidate in ("sol", "opus"):
        original = read_json(f"candidates/{candidate}/candidates.json")
        original_rows = original.get("rows", original.get("candidates"))
        assert len(original_rows) == 15
        for bucket in "ABCDE":
            assert sorted(
                row["rank"] for row in original_rows
                if row["category"].startswith(bucket)
            ) == [1, 2, 3]
        read_json(f"candidates/{candidate}/debate.json")
    manifest = read_json("source-evidence-manifest.json")
    archive = ROOT / manifest["archive"]
    assert hashlib.sha256(archive.read_bytes()).hexdigest() == manifest["archive_sha256"]
    expected = {row["archive_path"]: row for row in manifest["files"]}
    assert len(expected) == manifest["file_count"]
    for record in read_json("candidates/sol/source-inventory.json")["files"]:
        assert expected["sol/" + record["path"]]["sha256"] == record["sha256"]
    with tarfile.open(archive) as capture:
        assert {member.name for member in capture.getmembers()} == set(expected)
        for member in capture.getmembers():
            assert member.isfile()
            content = capture.extractfile(member).read()
            record = expected[member.name]
            assert len(content) == record["bytes"]
            assert hashlib.sha256(content).hexdigest() == record["sha256"]
    report = (ROOT / "recommendation.md").read_text()
    listed = re.findall(r"https://github.com/([^/]+/[^/]+)/pull/(\d+)", report)
    assert {(row["repository"], str(row["pr_number"])) for row in rows} <= set(listed)
    for target in re.findall(r"\]\(([^)]+)\)", report):
        if "://" not in target and not target.startswith("#"):
            assert (ROOT / target.split("#", 1)[0]).exists(), target
    probes = read_json("runtime-probes.json")["runs"]
    results = {(row["label"], row["code"]): row["exit_code"] for row in probes}
    assert results["16943-base", "from django import forms"] == 0
    assert results["16943-head", "from django import forms"] == 1
    assert results["16943-head", "from django.db import models; from django import forms"] == 0
    assert results["20724-base", "import django.template"] == 0
    assert results["20724-head", "import django.template"] == 1
    print(
        f"Verified 15 final tasks, 30 comparison pins, both frozen portfolios, "
        f"{len(expected)} archived source files, report links and import-probe outcomes."
    )


if __name__ == "__main__":
    verify()
