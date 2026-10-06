"""Prove a ruling's record is refused without its blind answers and that the cases kept for the user are computed."""
import copy
import json
from pathlib import Path
import unittest

import ruling_record

HERE = Path(__file__).parent
CLAUSES = json.loads((HERE / "two-questions.v3.clauses.json").read_text(encoding="utf-8"))
S9 = json.loads((HERE / "example-S9.json").read_text(encoding="utf-8"))


class RulingRecordTest(unittest.TestCase):
    def test_review_9_is_kept_for_the_user_and_recorded_as_a_surprise(self):
        self.assertEqual(ruling_record.faults(S9), [])
        stays = ruling_record.reasons(S9, CLAUSES)
        self.assertIn("the recommendation differs from a blind assessor's answer", stays)
        self.assertIn("clause P1b was written from the ruling under review", stays)
        self.assertIn("the recommendation would change a saved ruling", stays)
        self.assertTrue(any("two rules pointing different ways" in reason for reason in stays))
        surprises = ruling_record.surprises(S9, CLAUSES)
        self.assertIn("against the first recommendation", surprises)
        self.assertIn("asked 4 times before it was settled", surprises)
        self.assertIn("assessor-1 agrees through a clause written from this ruling; not counted as agreement", surprises)

    def test_a_record_without_two_blind_answers_from_another_family_is_refused(self):
        record = copy.deepcopy(S9)
        record["answers"][2]["family"] = "claude"
        self.assertIn("two blind answers from another model family", ruling_record.faults(record)[0])
        del record["answers"][1:]
        self.assertIn("two blind answers from another model family", ruling_record.faults(record)[0])

    def test_a_new_candidate_cannot_be_recorded_on_a_refused_dossier(self):
        # The open directories were written before the check; r-base-ui-5460 N1 has no promise.
        open_dossier = ("docs/research/"
                        "cohort-rebuild-2026-10-05/second-pass/candidates/r-base-ui-5460")
        record = {**copy.deepcopy(S9), "reconstructed": False, "group": "N1", "dossier": None}
        self.assertIn("needs `dossier`", ruling_record.faults(record)[0])
        record["dossier"] = open_dossier
        self.assertIn("the dossier is refused: N1: summary.json needs `promise`", ruling_record.faults(record)[0])

    def test_an_answer_must_name_its_clauses_and_nearest_rulings(self):
        record = copy.deepcopy(S9)
        del record["answers"][0]["nearest"]
        self.assertEqual(ruling_record.faults(record), ["recommender: needs nearest"])

    def test_an_agreed_answer_on_well_founded_clauses_gives_no_reason(self):
        record = copy.deepcopy(S9)
        record["reviews"] = None
        for answer in record["answers"]:
            answer.update(outcome="problem", clauses=["P6"], conflict=None)
        tested = {**CLAUSES, "P6": {**CLAUSES["P6"], "tested": True}}
        self.assertEqual(ruling_record.reasons(record, tested), [])
        self.assertEqual(ruling_record.reasons(record, CLAUSES), ["clause P6 has not been applied blind to a case it was not written from"])

    def test_a_shape_nobody_ruled_on_and_a_named_gap_are_reasons(self):
        record = copy.deepcopy(S9)
        record["answers"][1].update(nearest=[], rule_gap="the rule does not rank a guard against a new promise")
        stays = ruling_record.reasons(record, CLAUSES)
        self.assertIn("assessor-1 found no earlier ruling of this shape", stays)
        self.assertIn("assessor-1 names a gap in the rule: the rule does not rank a guard against a new promise", stays)


if __name__ == "__main__":
    unittest.main()
