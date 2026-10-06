"""Prove a ruling's record is refused without its blind answers, and that the cases kept for the user are computed."""
import copy
import json
from pathlib import Path
import re
import tempfile
import unittest

import ruling_record

ROOT = Path(__file__).resolve().parents[2]
RESEARCH = ROOT / "docs/research"
REVIEW_9 = RESEARCH / "cohort-rebuild-2026-10-05/second-pass/rulings/S9-ruling-30-R2a.before.json"


class RulingRecordTest(unittest.TestCase):
    def setUp(self):
        self.record = ruling_record.load(REVIEW_9)
        self.after = ruling_record.load(str(REVIEW_9).replace(".before.", ".after."))

    def test_review_9_is_kept_for_the_user_and_recorded_as_a_surprise(self):
        stays = ruling_record.reasons(self.record)
        for reason in ("the recommendation differs from a blind assessor's answer", "clause P1b was written from the ruling under review",
                       "the recommendation would change a saved ruling", "second-02 was decided under the earlier reading and has not been shown again"):
            self.assertIn(reason, stays)
        self.assertTrue(any("two rules pointing different ways" in reason for reason in stays))
        surprises = ruling_record.surprises(self.record, self.after)
        self.assertIn("against the first recommendation, which was stricter", surprises)
        self.assertIn("asked 4 times before it was settled", surprises)
        self.assertIn("assessor-1 agrees through a clause written from this ruling; not counted as agreement", surprises)

    def test_a_record_needs_two_blind_answers_from_another_family(self):
        self.record["answers"][2]["family"] = "claude"
        self.assertEqual(ruling_record.faults(self.record), ["the record needs two blind answers from another model family than the recommender's"])

    def test_names_outside_the_tables_are_refused(self):
        self.record["answers"][0].update(clauses=["P99"], nearest=["S9", "ruling 30"])
        self.assertEqual(ruling_record.faults(self.record), ["recommender: clause P99 is not in the clause table",
                                                             "recommender: ruling 30 is not a ruling in the index"])

    def test_a_new_candidate_cannot_be_recorded_on_a_refused_dossier(self):
        with tempfile.TemporaryDirectory() as dossier:
            Path(dossier, "summary.json").write_text(json.dumps([{"group": "N1", "kind": "candidate", "recommendation": "problem"}]), encoding="utf-8")
            self.record.update(reconstructed=False, group="N1", dossier=dossier,
                               rule={"path": self.record["rule"]["path"], "sha256": ruling_record.digest(ROOT / self.record["rule"]["path"])})
            self.assertIn("the dossier is refused: N1: needs `promise`", ruling_record.faults(self.record)[0])
            self.record["rule"]["sha256"] = "0" * 64
            self.assertIn("`rule` must pin the rule text", ruling_record.faults(self.record)[0])

    def test_a_recovery_question_and_an_unruled_shape_are_reasons(self):
        self.record.update(kind="recovery", reviews=None)
        self.record["answers"][1].update(nearest=[], outcome="cannot-tell")
        stays = ruling_record.reasons(self.record)
        self.assertIn("assessor-1 could not tell", stays)
        self.assertIn("a recovery decision is the user's under ADR-0006", stays)
        self.assertIn("assessor-1 found no earlier ruling of this shape", stays)

    def test_agreement_on_a_tested_clause_from_current_rulings_gives_no_reason(self):
        record = copy.deepcopy(self.record)
        record["reviews"] = None
        for answer in record["answers"]:
            answer.update(outcome="problem", clauses=["P6"], conflict=None, rule_gap=None, nearest=["R6"])
        self.assertEqual(ruling_record.reasons(record), ["clause P6 has not been applied blind to a case it was not written from"])

    def test_the_after_record_pins_the_first_answers(self):
        self.assertEqual(ruling_record.after_faults(self.record, self.after, REVIEW_9), [])
        self.after["before"]["sha256"] = "0" * 64
        self.assertIn("never edited", ruling_record.after_faults(self.record, self.after, REVIEW_9)[0])


class SavedRecordsTest(unittest.TestCase):
    def test_every_saved_record_is_sound(self):
        for path in sorted(RESEARCH.rglob("rulings/*.before.json")):
            record = ruling_record.load(path)
            self.assertEqual(ruling_record.faults(record), [], path)
            after = Path(str(path).replace(".before.", ".after."))
            if after.exists():
                self.assertEqual(ruling_record.after_faults(record, ruling_record.load(after), path), [], after)

    def test_every_second_pass_ruling_from_the_eleventh_has_its_record(self):
        rulings = RESEARCH / "cohort-rebuild-2026-10-05/second-pass/rulings"
        late = [path for path in rulings.glob("[0-9][0-9]-*.md") if int(path.name[:2]) >= 11]
        self.assertEqual([path.name for path in late if not path.with_suffix(".before.json").exists()], [])


if __name__ == "__main__":
    unittest.main()
