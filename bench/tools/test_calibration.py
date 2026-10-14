#!/usr/bin/env python3
"""Exercise calibration checks, blinded impact cards and the decision queue on disposable records.

Usage: python3 bench/tools/test_calibration.py
Inputs: the hand-checked fixture of test_current_grading.py; no network or model calls.
Exit codes: 0 all checks pass; 1 a test fails.
"""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import calibration
import current_grading as current
import evaluator_audit
from test_current_grading import approved, fixture, save_current, write

TOOL = Path(__file__).with_name("calibration.py")
CARD = "bench/grading/current/impact-cards/GT-t1.json"


def stratum(identifier):
    return {"id": identifier, "unit": "claim", "population": f"Every {identifier} unit"}


def pending(entries):
    return {"state": "pending", "reason": "Awaiting the human selection", "authority": None, "receipt": None,
            "receipt_scope": None, entries: []}


class CalibrationTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name).resolve()
        self.selected, self.documents = fixture(self.root)
        self.target = self.documents["reference"]["targets"][0]
        self.family = self.target["families"][0]
        self.boundary = self.root / "boundary.md"
        self.boundary.write_text("Serious means stored data is lost.\n", encoding="utf-8")
        self.card = {"schema_version": 1, "family": "GT-t1", "target": "t-example", "domain": "correctness",
                     "attribution": {"relation": "introduced", "reason": "The head removes the lock."},
                     "consequence": "A concurrent write is lost.", "exposure": "Two writers on one key.",
                     "controls": "No error is reported.", "reversibility": "The lost write cannot be recovered.",
                     "workload": None, "change_activity": None,
                     "grouping": {"state": "confirmed", "reason": "One mechanism."}, "limits": ["Shown with a test double."],
                     "evidence": [self.family["evidence"][0]]}
        self.decision = {"id": "AD-GT-t1-impact", "target": "t-example", "revision": self.target["revision"], "subject": "GT-t1",
                         "dimension": "impact", "status": "proposed", "outcome": "other-material", "authority": "automation",
                         "reason": "PROPOSER-RATIONALE", "receipt": None, "receipt_scope": None,
                         "boundary": current.pin_file(self.boundary, self.root), "evidence": [], "independent_checks": []}
        self.documents["adjudication"]["decisions"].append(self.decision)
        self.documents["audit"] = {"schema_version": 1, "state": "unassessed", "reason": "The sample audit has not run.",
                                   "plan": {"declared_at": "2026-10-03T00:00:00Z", "basis": "No grade is saved.", "seed": "fixture",
                                            "selection": "Hash order.", "strata": [stratum(s) for s in sorted(calibration.STRATA)],
                                            "sample": pending("sizes"), "tolerances": pending("limits")},
                                   "evidence": []}
        self.save()

    def tearDown(self):
        self.directory.cleanup()

    def save(self, queue=True):
        write(self.root, CARD, self.card)
        self.decision["evidence"] = [current.pin_file(self.root / CARD, self.root)]
        save_current(self.root, self.selected, self.documents)
        if queue:
            self.run_tool("queue", "--out", str(self.root / "bench/grading/current/decision-queue.json"))

    def run_tool(self, *arguments):
        return subprocess.run([sys.executable, str(TOOL), *arguments, "--root", str(self.root)],
                              capture_output=True, text=True, encoding="utf-8")

    def queue(self):
        return json.loads(self.run_tool("queue").stdout)

    def assert_problem(self, text):
        result = self.run_tool("check")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn(text, result.stdout)

    def independent_check(self, result):
        return {"source": current.pin_file(self.boundary, self.root), "checker": "second inspector",
                "independent_of": "proposer", "result": result, "reason": "INSPECTOR-RATIONALE"}

    def test_complete_records_pass_and_a_proposal_leaves_impact_unknown(self):
        result = self.run_tool("check")
        self.assertEqual((result.returncode, result.stdout.strip()),
                         (0, "calibration records valid; 0/1 families have an approved impact band"))
        row = self.queue()["families"][0]
        self.assertEqual((row["impact"]["band"], row["impact"]["decision"]["outcome"], row["impact"]["decision"]["status"]),
                         ("unknown", "other-material", "proposed"))
        self.assertEqual(row["needs"], ["eligibility ruling", "impact ruling"])

    def test_every_family_needs_one_impact_decision(self):
        self.documents["adjudication"]["decisions"].clear()
        self.save(queue=False)
        self.assert_problem("GT-t1: needs exactly one impact decision, approved or proposed; found 0")

    def test_impact_decision_pins_its_boundary_and_names_a_current_family(self):
        self.decision["boundary"] = None
        self.save(queue=False)
        self.assert_problem("AD-GT-t1-impact: an impact decision pins the boundary it was assessed under")
        self.decision["boundary"] = current.pin_file(self.boundary, self.root)
        self.documents["adjudication"]["decisions"].append({**self.decision, "id": "AD-GT-gone-impact", "subject": "GT-gone"})
        self.save(queue=False)
        self.assert_problem("GT-gone: impact decision names no current family")

    def test_serious_proposal_needs_an_independent_inspection_and_keeps_disagreement(self):
        self.decision["outcome"] = "serious"
        self.save()
        self.assert_problem("a serious proposal records an independent inspection")
        self.decision["independent_checks"] = [self.independent_check("refuted")]
        self.save()
        self.assertEqual(self.run_tool("check").returncode, 0)
        queue = self.queue()
        self.assertTrue(queue["families"][0]["impact"]["disagreement"])
        self.assertEqual((queue["summary"]["impact_disagreements"], queue["summary"]["impact"]["unknown"]), (1, 1))

    def test_same_party_inspection_is_not_independent(self):
        self.decision["outcome"] = "serious"
        self.decision["independent_checks"] = [{**self.independent_check("confirmed"), "checker": "proposer"}]
        self.save()
        self.assert_problem("a serious proposal records an independent inspection")

    def test_approved_band_and_reference_must_agree(self):
        ruling = approved(self.documents, self.root, "GT-t1", "impact", "other-material")
        self.documents["adjudication"]["decisions"].remove(self.decision)
        ruling.update(boundary=self.decision["boundary"], receipt_scope="GT-t1 for impact")
        self.decision = ruling
        self.save(queue=False)
        self.assert_problem("GT-t1: reference impact and AD-GT-t1-impact disagree about approval")
        self.family["impact"] = {"band": "other-material", "reason": "Saved human ruling", "adjudication": ruling["id"]}
        self.save()
        self.assertEqual(self.run_tool("check").stdout.strip(), "calibration records valid; 1/1 families have an approved impact band")
        self.assertEqual(self.queue()["families"][0]["needs"], ["eligibility ruling"])

    def admit(self):
        """OP-1, admitted with an approved label that pins its card."""
        admission = approved(self.documents, self.root, "OP-1", "pre-existing-admission", "admitted")
        label = approved(self.documents, self.root, "OP-1", "impact", "other-material")
        admission["receipt_scope"], label["receipt_scope"] = "OP-1 for pre-existing-admission", "OP-1 for impact"
        self.admitted_card = {**self.card, "family": "OP-1", "attribution": {"relation": "pre-existing", "reason": "The same at both commits."}}
        label.update(boundary=self.decision["boundary"], reason="OWNER-LABEL-REASON", independent_checks=[self.independent_check("confirmed")])
        self.problem = {**{field: self.family[field] for field in ("title", "obligation", "trigger", "mechanism", "grouping_reason", "evidence")},
                        "id": "OP-1", "consequence": "A saved write disappears.", "concerns": ["Reliability"],
                        "revision": self.target["revision"], "admission": admission["id"],
                        "impact": {"band": "other-material", "reason": "Saved human ruling", "adjudication": label["id"]}}
        self.target["pre_existing_problems"] = [self.problem]
        self.documents["audit"]["plan"]["strata"] += [stratum(s) for s in sorted(evaluator_audit.PRE_EXISTING_STRATA.values())]
        return label

    def save_admitted(self, label, queue=True):
        card = write(self.root, "bench/grading/current/impact-cards/OP-1.json", self.admitted_card)
        label["evidence"] = [current.pin_file(card, self.root)]
        self.save(queue)

    def test_an_admitted_pre_existing_problem_carries_an_approved_label_as_a_family_does(self):
        label = self.admit()
        self.save_admitted(label)
        result = self.run_tool("check")
        self.assertEqual((result.returncode, result.stdout.strip()), (0, "calibration records valid; 0/1 families have an approved "
                         "impact band; 1/1 pre-existing problems have an approved impact label"))
        row = self.queue()["pre_existing_problems"][0]
        self.assertEqual((row["impact"]["band"], row["impact"]["decision"]["status"], row["impact"]["disagreement"]),
                         ("other-material", "approved", False))
        label["independent_checks"] = [self.independent_check("refuted")]
        self.save_admitted(label)
        self.assertTrue(self.queue()["pre_existing_problems"][0]["impact"]["disagreement"])
        self.assertEqual(self.run_tool("check").returncode, 0)

    def test_every_label_of_a_pre_existing_problem_needs_an_independent_inspection(self):
        label = self.admit()
        label["independent_checks"] = []
        self.save_admitted(label, queue=False)
        self.assert_problem("AD-OP-1-impact: the label of a pre-existing problem needs an independent inspection")
        label["independent_checks"] = [{**self.independent_check("confirmed"), "checker": "proposer"}]
        self.save_admitted(label, queue=False)
        self.assert_problem("AD-OP-1-impact: the label of a pre-existing problem needs an independent inspection")
        self.problem["impact"] = {"band": "unknown", "reason": "Awaiting the label", "adjudication": None}
        label.update(status="proposed", authority="automation", receipt=None, receipt_scope=None, independent_checks=[])
        self.save_admitted(label, queue=False)
        self.assert_problem("AD-OP-1-impact: the label of a pre-existing problem records an independent inspection, including disagreement")

    def test_an_admitted_pre_existing_problem_without_an_approved_label_is_a_problem(self):
        label = self.admit()
        self.problem["impact"] = {"band": "unknown", "reason": "Awaiting the label", "adjudication": None}
        label.update(status="proposed", authority="automation", receipt=None, receipt_scope=None)
        self.save_admitted(label, queue=False)
        self.assert_problem("OP-1: an admitted pre-existing problem needs an approved impact label")
        self.documents["adjudication"]["decisions"].remove(label)
        self.save(queue=False)
        self.assert_problem("OP-1: needs exactly one impact decision, approved or proposed; found 0")

    def test_only_a_pre_existing_problems_card_attributes_its_fault_as_pre_existing(self):
        label = self.admit()
        self.admitted_card["attribution"]["relation"] = "introduced"
        self.save_admitted(label, queue=False)
        self.assert_problem("impact-cards/OP-1.json: the card of a pre-existing problem, and no other, attributes its fault as pre-existing")
        self.admitted_card["attribution"]["relation"] = "pre-existing"
        self.card["attribution"]["relation"] = "pre-existing"
        self.save_admitted(label, queue=False)
        self.assert_problem("impact-cards/GT-t1.json: the card of a pre-existing problem, and no other, attributes its fault as pre-existing")

    def test_a_pre_existing_problems_blinded_card_omits_its_label_and_the_ruling_that_holds_it(self):
        label = self.admit()
        self.save_admitted(label)
        out, key = self.root / "cards", self.root / "cards.key.json"
        result = self.run_tool("cards", "--out", str(out), "--key", str(key))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        text = (out / "OP-1.md").read_text(encoding="utf-8")
        for shown in ("## Pre-existing problem", "Admission: Admitted by a saved human ruling.", "Attribution (pre-existing): The same at both commits."):
            self.assertIn(shown, text)
        for hidden in ("OWNER-LABEL-REASON", "other-material\n", "R1", "Eligibility"):
            self.assertNotIn(hidden, text)
        self.assertEqual(list(json.loads(key.read_text(encoding="utf-8"))["OP-1"]), ["E1"])

    def test_performance_and_maintenance_cards_name_their_cost_and_activity(self):
        for domain, field, problem in (("performance", "workload", "names its workload and cost"),
                                       ("architecture-maintenance", "change_activity", "names a concrete change activity")):
            with self.subTest(domain=domain):
                self.card.update(domain=domain, workload=None, change_activity=None)
                self.save(queue=False)
                self.assert_problem(problem)
                self.card[field] = "Validating one 1000-field query: 81 ms to 2703 ms."
                self.save()
                self.assertEqual(self.run_tool("check").returncode, 0)

    def test_card_must_be_the_family_card_and_match_its_pin(self):
        self.card["family"] = "GT-other"
        self.save(queue=False)
        self.assert_problem("card belongs to another family or target")
        self.card["family"] = "GT-t1"
        self.save()
        (self.root / CARD).write_text(json.dumps({**self.card, "consequence": "Edited after pinning."}), encoding="utf-8")
        self.assert_problem("source hash changed")

    def test_whole_receipt_is_not_a_scope(self):
        ruling = approved(self.documents, self.root, "CL-t1")
        self.documents["claim"]["claims"][0].update(adjudication=ruling["id"])
        ruling["outcome"] = "advisory"
        self.save(queue=False)
        self.assert_problem("AD-CL-t1-eligibility: receipt scope is the whole receipt")
        ruling["receipt_scope"] = "Human approves CL-t1"
        self.save()
        self.assertEqual(self.run_tool("check").returncode, 0)

    def empty_control(self):
        self.target["families"] = []
        self.target["control"].update(status="unaudited", reason="No audit decision.")
        self.documents["claim"]["claims"].clear()
        self.documents["adjudication"]["decisions"].clear()
        decision = {**self.decision, "id": "AD-t-example-control", "subject": "t-example", "dimension": "control",
                    "outcome": "provisional", "boundary": None, "evidence": [self.family["evidence"][0]],
                    "independent_checks": [self.independent_check("confirmed")]}
        self.documents["adjudication"]["decisions"].append(decision)
        return decision

    def save_control(self, queue=True):
        save_current(self.root, self.selected, self.documents)
        if queue:
            self.run_tool("queue", "--out", str(self.root / "bench/grading/current/decision-queue.json"))

    def test_empty_reference_control_needs_an_audited_decision(self):
        decision = self.empty_control()
        self.save_control()
        self.assertEqual(self.run_tool("check").returncode, 0)
        row = self.queue()["controls"][0]
        self.assertEqual((row["status"], row["needs"], row["decision"]["status"]), ("unaudited", ["control ruling"], "proposed"))
        self.assertEqual([m["state"] for m in self.queue()["measures"] if m["measure"] == "clean-control rate"], ["blocked"])
        decision.update(status="approved", authority="human", receipt=current.pin_file(self.boundary, self.root),
                        receipt_scope="Serious means")
        self.save_control(queue=False)
        self.assert_problem("t-example: control status differs from approved AD-t-example-control")
        decision.update(status="proposed", authority="automation", receipt=None, receipt_scope=None, independent_checks=[])
        self.save_control(queue=False)
        self.assert_problem("records its audit evidence and an independent audit")
        self.documents["adjudication"]["decisions"].clear()
        self.save_control(queue=False)
        self.assert_problem("t-example: an empty-reference control needs exactly one control decision; found 0")

    def test_audit_plan_covers_required_strata(self):
        self.documents["audit"]["plan"]["strata"].pop()
        self.save(queue=False)
        self.assert_problem("audit plan: strata must be unique and cover")

    def test_audit_selection_needs_the_saved_human_selection(self):
        sample = self.documents["audit"]["plan"]["sample"]
        sample["sizes"] = [{"stratum": s, "size": 5} for s in sorted(calibration.STRATA)]
        self.save(queue=False)
        self.assert_problem("audit sample: a pending selection records no values or authority")
        receipt = self.root / "audit-receipt.md"
        receipt.write_text("> Five per stratum.\n", encoding="utf-8")
        sample.update(state="selected", authority="human", receipt=current.pin_file(receipt, self.root), receipt_scope="Ten per stratum.")
        self.save(queue=False)
        self.assert_problem("audit sample: selection scope is not in the saved receipt")
        sample["receipt_scope"] = "> Five per stratum."
        sample["sizes"].pop()
        self.save(queue=False)
        self.assert_problem("audit sample: selected values cover every stratum once")
        sample["sizes"].append({"stratum": sorted(calibration.STRATA)[-1], "size": "all"})
        self.save()
        self.assertEqual(self.run_tool("check").returncode, 0)
        self.assertEqual(self.queue()["audit"]["needs"], ["human-selected tolerances"])

    def test_superseded_round_keeps_its_own_seed_and_its_closure_in_the_receipt(self):
        receipt = self.root / "audit-receipt.md"
        receipt.write_text("Decision: the round drawn with seed fixture-round-one is closed as superseded.\n", encoding="utf-8")
        closed = {"closed_at": "2026-10-12T00:00:00Z", "reason": "A new rubric replaced its labels.", "authority": "human",
                  "receipt": current.pin_file(receipt, self.root),
                  "receipt_scope": "Decision: the round drawn with seed fixture-round-one is closed as passed.",
                  "plan": json.loads(json.dumps(self.documents["audit"]["plan"])), "evidence": []}
        self.documents["audit"]["superseded"] = [closed]
        self.save(queue=False)
        self.assert_problem("audit: the declared plan and each superseded round have their own seed")
        self.assert_problem("audit: the closure of fixture names the seed of the round it closes")
        self.assert_problem("audit: the closure of fixture is not in the saved receipt")
        closed["plan"]["seed"] = "fixture-round-one"
        closed["receipt_scope"] = "Decision: the round drawn with seed fixture-round-one is closed as superseded."
        self.save()
        self.assertEqual(self.run_tool("check").returncode, 0)

    def test_superseded_plan_keeps_its_own_strata_and_selection(self):
        receipt = self.root / "audit-receipt.md"
        receipt.write_text("Closed: fixture-round-one. One unit in the old stratum.\n", encoding="utf-8")
        pin = current.pin_file(receipt, self.root)
        old = {"declared_at": "2026-10-03T00:00:00Z", "basis": "No grade was saved.", "seed": "fixture-round-one",
               "selection": "Hash order.", "strata": [stratum("old-label")],
               "sample": {"state": "selected", "reason": "Saved selection", "authority": "human", "receipt": pin,
                          "receipt_scope": "One unit in the old stratum.", "sizes": []},
               "tolerances": pending("limits")}
        self.documents["audit"]["superseded"] = [{
            "closed_at": "2026-10-12T00:00:00Z", "reason": "A new rubric replaced its labels.", "authority": "human",
            "receipt": pin, "receipt_scope": "Closed: fixture-round-one.", "plan": old, "evidence": []}]
        self.save(queue=False)
        self.assert_problem("audit superseded fixture-round-one sample: selected values cover every stratum once")
        old["sample"]["sizes"] = [{"stratum": "old-label", "size": 1}]
        old["strata"].append(stratum("old-label"))
        self.save(queue=False)
        self.assert_problem("audit superseded fixture-round-one: strata must be unique and cover []")
        old["strata"].pop()
        self.save()
        self.assertEqual(self.run_tool("check").returncode, 0)

    def test_audit_without_a_plan_is_refused_by_every_operation(self):
        self.documents["audit"] = {"state": "unassessed", "evidence": []}
        self.save(queue=False)
        for operation in ("check", "queue"):
            result = self.run_tool(operation)
            self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
            self.assertIn("current audit: ", result.stdout)

    def test_assessed_audit_needs_selected_plan_and_evidence(self):
        self.documents["audit"]["state"] = "assessed"
        self.save(queue=False)
        self.assert_problem("audit: an assessed audit needs the selected plan and its saved evidence")

    def test_saved_queue_must_match_the_records(self):
        self.decision["reason"] = "Changed after the queue was saved"
        self.save(queue=False)
        self.assert_problem("decision-queue.json differs from the current records")

    def test_blinded_cards_omit_proposals_inspections_and_source_paths(self):
        self.decision["independent_checks"] = [self.independent_check("refuted")]
        self.save()
        out, key = self.root / "cards", self.root / "cards.key.json"
        result = self.run_tool("cards", "--out", str(out), "--key", str(key))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        text = (out / "GT-t1.md").read_text(encoding="utf-8")
        self.assertIn("A concurrent write is lost.", text)
        self.assertIn("Pending a human ruling", text)
        for hidden in ("PROPOSER-RATIONALE", "INSPECTOR-RATIONALE", "other-material\n", "bench/targets", "proposed"):
            self.assertNotIn(hidden, text)
        self.assertEqual(json.loads(key.read_text(encoding="utf-8"))["GT-t1"]["E1"], self.family["evidence"][0])
        self.assertEqual(self.run_tool("cards", "--out", str(out), "--key", str(self.root / "other.json")).returncode, 1)
        inside = self.run_tool("cards", "--out", str(self.root / "fresh"), "--key", str(self.root / "fresh/key.json"))
        self.assertEqual((inside.returncode, inside.stdout.strip()), (1, "keep the key outside the card directory"))
        self.assertFalse((self.root / "fresh").exists())

    def test_blinded_cards_refuse_a_reviewer_identity(self):
        self.card["consequence"] = "The example arm found a lost write."
        self.save()
        result = self.run_tool("cards", "--out", str(self.root / "cards"), "--key", str(self.root / "cards.key.json"))
        self.assertEqual(result.returncode, 1)
        self.assertIn("GT-t1: impact card exposes reviewer identities or private paths: example", result.stdout)
        self.assertFalse((self.root / "cards").exists())

    def test_unreadable_current_records_exit_two(self):
        (self.root / "bench/grading/current/audits.json").unlink()
        result = self.run_tool("check")
        self.assertEqual(result.returncode, 2)
        self.assertIn("calibration.py: cannot read", result.stderr)

    def test_examples_conform_to_their_schemas(self):
        for kind in ("impact-card", "audit"):
            current.validate_schema(kind, current.read_json(current.BENCH / f"schema/examples/current-{kind}.example.json"))


if __name__ == "__main__":
    unittest.main()
