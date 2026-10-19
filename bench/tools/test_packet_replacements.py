#!/usr/bin/env python3
"""Check that the decisions the last-push re-cut selection stages follow the owner's rule.
test_review_code_selection.py checks that the published selection follows from it.

Usage: python3 bench/tools/test_packet_replacements.py
Inputs: the committed record under docs/research/last-push-recut-2026-10-07.
No network.
Exit codes: 0 all checks pass; 1 a test fails.
"""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
RECORD = Path("docs/research/last-push-recut-2026-10-07")


def record_script(name: str):
    spec = importlib.util.spec_from_file_location(f"recut_{name}", ROOT / RECORD / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


stage_edition = record_script("stage_edition")

KEEP = "re-cut with the reviews kept as a recorded deviation"
LOST_A_RECORD, LOST_NONE = "i-requests-6667", "u-grpc-go-6919"


def read(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def owner_decision_problems(decisions: dict, replacements: dict, tasks: list) -> list:
    """The owner's rule that the retired verify.py checked, applied to the decisions staged_registry() reads:
    a task that lost a record is decided re-cut and run again; one that lost none is decided either way."""
    groups = {group["id"]: group for group in decisions["groups"]}
    decided = {entry["target"]: groups.get(entry["group"], {}) for entry in decisions["tasks"]}
    removed = {entry["target"]: entry["records_removed"] for entry in replacements["targets"]}
    found = []
    if sorted(decided) != sorted(tasks) or len(decisions["tasks"]) != len(tasks):
        found.append("the decisions do not give each staged task one entry")
    for task in tasks:
        group = decided.get(task, {})
        if removed[task]:
            if group.get("status") != "decided" or group.get("decision") != stage_edition.RERUN:
                found.append(f"{task}: lost a record but is not recorded as decided, re-cut and run again")
        elif group.get("status") != "decided" or group.get("decision") not in (stage_edition.RERUN, KEEP):
            found.append(f"{task}: lost no record and has no decision the owner made")
    return found


class OwnerDecisionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.decisions = read(stage_edition.HERE / "decisions.v1.json")
        self.replacements = read(stage_edition.HERE / "packet-replacements.v1.json")
        self.tasks = [task["id"] for task in read(stage_edition.BEFORE)["tasks"]]

    def problems(self) -> list:
        return owner_decision_problems(self.decisions, self.replacements, self.tasks)

    def group_of(self, target: str) -> dict:
        name = next(entry["group"] for entry in self.decisions["tasks"] if entry["target"] == target)
        return next(group for group in self.decisions["groups"] if group["id"] == name)

    def test_the_committed_decisions_follow_the_owners_rule(self) -> None:
        self.assertEqual(self.problems(), [])

    def test_a_task_that_lost_a_record_and_is_kept_is_reported(self) -> None:
        entries = {entry["target"]: entry for entry in self.decisions["tasks"]}
        entries[LOST_A_RECORD]["group"], entries[LOST_NONE]["group"] = entries[LOST_NONE]["group"], entries[LOST_A_RECORD]["group"]
        self.assertEqual(self.problems(), [f"{LOST_A_RECORD}: lost a record but is not recorded as decided, re-cut and run again"])

    def test_a_task_whose_decision_is_not_the_owners_is_reported(self) -> None:
        self.group_of(LOST_NONE).update(status="proposed", decision=None)
        self.assertIn(f"{LOST_NONE}: lost no record and has no decision the owner made", self.problems())

    def test_a_task_without_a_decision_is_reported(self) -> None:
        self.decisions["tasks"].pop()
        self.assertIn("the decisions do not give each staged task one entry", self.problems())


if __name__ == "__main__":
    unittest.main(verbosity=2)
