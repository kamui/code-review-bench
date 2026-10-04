#!/usr/bin/env python3
"""Drive evaluator_audit.py on the synthetic cohort of test_grade.py with manual assessors.

Usage: python3 bench/tools/test_evaluator_audit.py
Inputs: disposable records and a stub provisioner; no network or model calls.
Exit codes: 0 all checks pass; 1 a test fails.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
from test_current_grading import approved  # noqa: E402
from test_grade import RUN, TARGET, Grade, claim, grade, remedy, reviewed, write_json  # noqa: E402
import unittest  # noqa: E402

TOOL = TOOLS / "evaluator_audit.py"
STRATA = {"recovery": "family-review", "non-recovery": "family-review", "unresolved-recovery": "family-review",
          "serious-reference": "family-review", "refuted": "claim", "unsupported": "claim", "advisory": "claim",
          "unresolved-claim": "claim", "other-below-threshold": "claim"}
UNIT = f"{RUN}/att-001"


def selection(entries, field, value):
    return {"state": "selected", "reason": "Fixture selection", "authority": "human", "receipt": None, "receipt_scope": None,
            entries: [{"stratum": name, field: value} for name in STRATA]}


class Audit(Grade):
    def setUp(self):
        super().setUp()
        for family in ("GT-t1", "GT-t2"):
            self.cohort.approve_family(family)
        decision = approved(self.cohort.documents, self.root, "GT-t1", "impact", "serious")
        decision.update(boundary=decision["receipt"], independent_checks=[{
            "source": decision["receipt"], "checker": "independent", "independent_of": "primary", "result": "confirmed",
            "reason": "Scope checked"}])
        self.cohort.families()[0]["impact"] = {"band": "serious", "reason": "Saved ruling", "adjudication": decision["id"]}
        self.cohort.documents["audit"] = {
            "schema_version": 1, "state": "unassessed", "reason": "Not audited.", "evidence": [],
            "plan": {"declared_at": "2026-01-01T00:00:00Z", "basis": "No grade is saved.", "seed": "fixture", "selection": "Hash order.",
                     "strata": [{"id": name, "unit": unit, "population": name} for name, unit in STRATA.items()],
                     "sample": selection("sizes", "size", 2), "tolerances": selection("limits", "max_errors", 0)}}
        self.cohort.save()
        self.current = self.root / "bench/grading/current"
        self.round = self.current / "audit/fixture"

    def audit(self, *arguments):
        return subprocess.run([sys.executable, str(TOOL), *arguments, "--root", str(self.root)],
                              capture_output=True, text=True, encoding="utf-8")

    def assessed(self, work, key, name, third="refuted", first="eligible", third_claims=None):
        """A prepared workspace holding ``name``'s verdicts on att-001: ``first`` and ``third`` are the outcomes of
        its first and third items, and ``third_claims`` replaces the third item's single claim."""
        tokens = {review["attempt_id"]: review["token"] for review in self.prepared(work, key)["reviews"]}
        caught = first == "eligible"
        write_json(work / "verdicts.json", {"new_candidates": [], "link_disputes": [], "reviews": {
            tokens["att-001"]: reviewed([claim("c1", "Races on close", first, "GT-t1" if caught else None)],
                                        [claim("c2", "Rename x", "advisory")],
                                        third_claims or [claim("c3", "Lock order is new", third)],
                                        recommendations=[remedy("r1", [(1, "Hold the lock while closing.")], ["c1"],
                                                                [("GT-t1", "unassessed")] if caught else [])]),
            tokens["att-002"]: reviewed([claim("c1", "Leaks the conn", "unsupported")]),
            tokens["att-003"]: reviewed()}})
        write_json(self.root / f"{name}.json", {"assessor": name, "method": "Read the workspace.", "completed_at": "2026-01-02T00:00:00Z"})
        return ["--work", str(work), "--key", str(key), "--assessor", str(self.root / f"{name}.json")]

    def graded(self):
        done = grade("map", "--root", str(self.root), *self.assessed(self.work, self.key, "first"))
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)

    def drawn(self):
        self.graded()
        done = self.audit("draw")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        return json.loads((self.round / "sample.json").read_text())

    def compared(self, **second):
        self.drawn()
        saved = [(self.current / name).read_bytes() for name in ("grades.json", "candidates.json")]
        done = self.audit("second", *self.assessed(self.root / "work-2", self.root / "keys/key-2.json", "second", **second))
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertEqual([(self.current / name).read_bytes() for name in ("grades.json", "candidates.json")], saved)
        done = self.audit("compare")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        return json.loads((self.round / "comparison.json").read_text())

    def reconcile(self, finding):
        write_json(self.root / "reconciliation.json", {"reconciler": "fixture", "decisions": [
            {"unit": f"{UNIT}#c3", "finding": finding, "reason": "Read main.go.", "evidence": ["main.go"]}]})
        return self.audit("conclude", "--reconciliation", str(self.root / "reconciliation.json"))

    def test_no_sample_is_drawn_before_every_batch_is_graded(self):
        done = self.audit("draw")
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn("no current grade", done.stdout)
        self.assertFalse((self.current / "audit").exists())

    def test_strata_hold_admitted_family_units_and_every_graded_claim_in_seeded_order(self):
        sample = self.drawn()
        strata = {stratum["id"]: stratum for stratum in sample["strata"]}
        self.assertEqual(strata["recovery"]["population"], [f"{UNIT}#GT-t1"])
        self.assertEqual(strata["non-recovery"]["population"],
                         [f"{UNIT}#GT-t2", f"{RUN}/att-003#GT-t1", f"{RUN}/att-003#GT-t2"])
        self.assertEqual(strata["serious-reference"]["population"], [f"{UNIT}#GT-t1", f"{RUN}/att-003#GT-t1"])
        self.assertEqual(strata["unsupported"]["population"], [f"{RUN}/att-002#c1"])
        self.assertEqual(strata["non-recovery"]["drawn"], sorted(
            strata["non-recovery"]["population"],
            key=lambda unit: hashlib.sha256(f"fixture:non-recovery:{unit}".encode()).hexdigest())[:2])
        twice = next(unit for unit in sample["units"] if unit["id"] == f"{UNIT}#GT-t1")
        self.assertEqual(twice["strata"], ["recovery", "serious-reference"])
        self.assertEqual(sample["batches"], [{"run": f"runs/{RUN}", "target": TARGET, "units": len(sample["units"])}])
        self.assertEqual(self.audit("draw").returncode, 1)

    def test_a_second_assessment_replaces_no_grade_and_its_disagreement_is_counted(self):
        comparison = self.compared(third="advisory")
        refuted = next(stratum for stratum in comparison["strata"] if stratum["id"] == "refuted")
        self.assertEqual(refuted, {"id": "refuted", "units": 1, "agreements": 0, "confusion": {"refuted": {"advisory": 1}}})
        self.assertEqual([row["id"] for row in comparison["units"] if not row["agreement"]], [f"{UNIT}#c3"])
        self.assertFalse((self.root / "work-2/clone").exists())

    def test_a_quotation_inside_the_first_claim_is_compared(self):
        comparison = self.compared(third_claims=[claim("c3", "Lock order", "advisory")])
        row = next(row for row in comparison["units"] if row["id"] == f"{UNIT}#c3")
        self.assertEqual((row["second"]["outcome"], row["agreement"]), ("advisory", False))

    def test_a_claim_the_second_assessor_split_elsewhere_is_unmatched(self):
        comparison = self.compared(third_claims=[claim("c3", "It breaks.", "refuted"), claim("c4", "It", "refuted")])
        row = next(row for row in comparison["units"] if row["id"] == f"{UNIT}#c3")
        self.assertEqual((row["second"]["outcome"], row["agreement"]), ("unmatched", False))
        self.assertEqual([c["id"] for c in row["second"]["claims"]], ["c3", "c4"])

    def test_a_recovery_the_second_assessor_denies_is_a_disagreement_in_both_its_strata(self):
        comparison = self.compared(first="refuted")
        strata = {stratum["id"]: stratum for stratum in comparison["strata"]}
        self.assertEqual(strata["recovery"]["confusion"], {"caught": {"missed": 1}})
        self.assertEqual(strata["serious-reference"]["agreements"], strata["serious-reference"]["units"] - 1)
        self.assertEqual(strata["non-recovery"]["agreements"], strata["non-recovery"]["units"])

    def test_a_batch_graded_again_after_the_draw_is_not_compared(self):
        self.drawn()
        second = self.assessed(self.root / "work-2", self.root / "keys/key-2.json", "second")
        done = grade("map", "--root", str(self.root), *self.assessed(self.root / "work-3", self.root / "keys/key-3.json", "first", "advisory"))
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        for operation in (("second", *second), ("compare",)):
            done = self.audit(*operation)
            self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
            self.assertIn("grades.json changed since the sample was drawn", done.stdout)
        self.assertFalse((self.round / "second").exists())

    def test_the_assessor_that_produced_the_grade_cannot_audit_it(self):
        self.drawn()
        done = self.audit("second", *self.assessed(self.root / "work-2", self.root / "keys/key-2.json", "first"))
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn("cannot be the second assessor", done.stdout)
        self.assertFalse((self.round / "second").exists())

    def test_comparison_waits_for_every_sampled_batch(self):
        self.drawn()
        done = self.audit("compare")
        self.assertEqual(done.returncode, 1)
        self.assertIn("no second assessment", done.stdout)

    def test_an_error_beyond_tolerance_leaves_the_audit_unassessed(self):
        self.compared(third="advisory")
        done = self.reconcile("first-error")
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn("refuted", done.stdout)
        self.assertEqual(json.loads((self.current / "audits.json").read_text())["state"], "unassessed")

    def test_an_undetermined_disagreement_saves_nothing_and_can_be_settled_later(self):
        self.compared(third="advisory")
        done = self.reconcile("undetermined")
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn("undetermined, so nothing was saved", done.stdout)
        self.assertFalse((self.round / "reconciliation.json").exists())
        self.assertEqual(self.reconcile("first-correct").returncode, 0)

    def test_a_failed_round_is_kept_and_a_new_seed_draws_a_fresh_sample(self):
        self.compared(third="advisory")
        self.assertEqual(self.reconcile("first-error").returncode, 1)
        self.assertEqual(self.reconcile("first-correct").returncode, 1)
        audit = json.loads((self.current / "audits.json").read_text())
        audit["plan"]["seed"] = "fixture-2"
        write_json(self.current / "audits.json", audit)
        done = self.audit("draw")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertTrue((self.round / "conclusion.json").exists())
        self.assertTrue((self.current / "audit/fixture-2/sample.json").exists())

    def test_every_disagreement_needs_a_reconciliation(self):
        self.compared(third="advisory")
        write_json(self.root / "reconciliation.json", {"reconciler": "fixture", "decisions": []})
        done = self.audit("conclude", "--reconciliation", str(self.root / "reconciliation.json"))
        self.assertEqual(done.returncode, 1)
        self.assertIn(f"{UNIT}#c3: a disagreement without a reconciliation", done.stdout)
        self.assertFalse((self.round / "conclusion.json").exists())

    def test_a_reconciled_audit_within_tolerance_is_assessed_with_pinned_evidence(self):
        self.compared(third="advisory")
        done = self.reconcile("first-correct")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        audit = json.loads((self.current / "audits.json").read_text())
        self.assertEqual(audit["state"], "assessed")
        self.assertIn("bench/grading/current/audit/fixture/sample.json", [pin["path"] for pin in audit["evidence"]])
        self.assertEqual(self.check().returncode, 0, self.check().stdout)


if __name__ == "__main__":
    unittest.main()
