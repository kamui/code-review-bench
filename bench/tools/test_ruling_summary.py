"""Check summary accounting and the evidence passed to the next assessor."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import ruling_record
import ruling_summary
from test_ruling_record import ROOT, sound


def pair(number, target="pr-1", confidence="high", decision="recovers", round_name="first"):
    before = sound("recovery")
    before.update(ruling=f"case-{number}", target=target, group=f"Q{number}")
    for answer in before["answers"]:
        answer.update(confidence=confidence, would_settle=confidence == "high")
    after = {"outcome": decision, "asked": 1, "settled_by": "owner", "ground": None, "miss": None, "lesson": None}
    return {"before": before, "after": after, "path": f"docs/research/{round_name}/rulings/{number:02d}.before.json",
            "hash": f"{number:064x}", "round": round_name, "opened": "2026-10-01", "closed": True}


class SummaryTest(unittest.TestCase):
    def test_eight_decisions_on_three_pull_requests_have_hand_counted_tables(self):
        pairs = [pair(1), pair(2), pair(3, "pr-2", "medium"), pair(4, "pr-2", decision="does-not-recover"),
                 pair(5, "pr-3"), pair(6, "pr-3"), pair(7, "pr-3"), pair(8, "pr-3")]
        pairs[3]["after"].update(miss={"cause": "fact-not-fetched", "note": "The fact was absent."},
                                  lesson={"says": "Fetch the fact.", "goes_to": "docs/ruling-dossier-brief.md"})
        pairs[5]["before"]["answers"][1].update(outcome="does-not-recover", confidence="medium", would_settle=False)
        pairs[6]["after"].update(asked=2, miss={"cause": "fact-not-fetched", "note": "The fact was absent again."})
        pairs[7]["before"].update(reconstructed=True)
        summary = ruling_summary.summarize(pairs)["contract_2"]["recovery"]
        self.assertEqual(summary["decisions"], 8)
        self.assertEqual(summary["pull_requests"], ["pr-1", "pr-2", "pr-3"])
        self.assertEqual(summary["parties"]["recommender"], {
            "high": {"answers": 7, "matches": 6, "excluded": 0},
            "medium": {"answers": 1, "matches": 1, "excluded": 0}})
        self.assertEqual(summary["parties"]["assessor-1"], {
            "high": {"answers": 6, "matches": 5, "excluded": 0},
            "medium": {"answers": 2, "matches": 1, "excluded": 0}})
        self.assertEqual(summary["blind_high_matches"], 5)
        self.assertEqual(summary["unanimous_matches"], 5)
        self.assertEqual(summary["earned"]["count"], 2)
        self.assertEqual(summary["earned"]["pull_requests"], ["pr-3"])
        self.assertEqual(summary["earned"]["level"], 0)
        self.assertEqual(summary["misses"], {"fact-not-fetched": 2})
        self.assertEqual(summary["lessons"][1], {"path": pairs[6]["path"], "cause_returned_after": pairs[3]["path"]})
        self.assertEqual(summary["ask_reasons"]["no policy is adopted, so agents settle nothing beyond ADR-0006"], 8)

    def test_contract_one_and_reconstructed_answers_do_not_earn_trust(self):
        pairs = [pair(1), pair(2), pair(3)]
        pairs[0]["before"]["contract"] = 1
        pairs[1]["before"]["reconstructed"] = True
        summary = ruling_summary.summarize(pairs)
        self.assertEqual(summary["baseline"]["recovery"]["decisions"], 1)
        self.assertEqual(summary["contract_2"]["recovery"]["earned"]["count"], 1)

    def test_model_change_starts_count_again_and_levels_need_distinct_pull_requests(self):
        pairs = [pair(number, f"pr-{number % 4}") for number in range(1, 16)]
        self.assertEqual(ruling_summary.earned(pairs, "recovery", ROOT)["level"], 1)
        pairs[-1]["before"]["answers"][1]["model"] = "replacement"
        self.assertEqual(ruling_summary.earned(pairs, "recovery", ROOT)["count"], 1)
        self.assertEqual(ruling_summary.level(30, set("abcde")), 2)
        self.assertEqual(ruling_summary.level(30, set("abc")), 0)

    def test_blind_agreement_from_a_clause_written_from_this_ruling_is_excluded(self):
        item = pair(1)
        item["before"].update(ruling="first-30", reviews={"ruling": "first-30", "saved_outcome": "suggestion"},
                              clauses="docs/research/cohort-rebuild-2026-10-05/second-pass/terms/two-questions.v4.clauses.json")
        for answer in item["before"]["answers"]:
            answer["clauses"] = ["P1b"]
        summary = ruling_summary.summarize([item])["contract_2"]["recovery"]
        self.assertEqual(summary["parties"]["assessor-1"]["high"]["excluded"], 1)
        self.assertEqual(summary["earned"]["count"], 0)

    def test_round_filter_keeps_final_decisions_from_later_reviews(self):
        first, review = pair(1), pair(2, decision="does-not-recover", round_name="second")
        first["before"]["ruling"] = "second-26"
        review["before"].update(ruling="second-26", group="Q1", reviews={"ruling": "S11", "saved_outcome": "recovers"})
        review["opened"] = "2026-10-02"
        summary = ruling_summary.summarize([first, review], round_name="first")["contract_2"]["recovery"]
        self.assertEqual(summary["parties"]["recommender"]["high"]["matches"], 0)
        self.assertEqual(summary["earned"]["count"], 0)

    def test_summary_reports_pending_and_rejects_unknown_round(self):
        pending = pair(1)
        pending["after"] = None
        self.assertEqual(ruling_summary.summarize([pending])["pending"], [pending["path"]])
        with self.assertRaisesRegex(ValueError, "unknown round"):
            ruling_summary.summarize([pending], round_name="absent")

    def test_level_changes_only_between_rounds_and_a_review_miss_resets_immediately(self):
        pairs = [pair(number, f"pr-{number % 4}") for number in range(1, 16)]
        first = pairs[0]
        first["before"]["ruling"] = "second-26"
        current = pair(16, "pr-5", round_name="second")
        current["opened"] = "2026-10-02"
        self.assertEqual(ruling_summary.summarize([*pairs, current], round_name="second")["contract_2"]["recovery"]["round_level"], 1)
        review = pair(17, "pr-1", decision="does-not-recover", round_name="second")
        review["opened"] = "2026-10-02"
        review["before"].update(ruling="second-26", group="Q1", reviews={"ruling": "second-26", "saved_outcome": "recovers"})
        review["before"]["answers"][1]["confidence"] = "medium"
        earned = ruling_summary.earned([*pairs, current, review], "recovery", ROOT)
        self.assertEqual(earned["count"], 0)
        self.assertEqual(earned["resets"][-1]["path"], review["path"])

    def test_the_plan_historical_cohort_is_reported_without_changing_saved_records(self):
        pairs = [item for item in ruling_summary.records() if item["before"].get("contract", 1) == 1
                 and not item["before"]["reconstructed"] and 11 <= int(Path(item["path"]).name[:2]) <= 24]
        baseline = ruling_summary.summarize(pairs)["baseline"]
        self.assertEqual(sum(entry["decisions"] for entry in baseline.values()), 14)
        self.assertEqual(sum(counts["matches"] for entry in baseline.values() for counts in entry["parties"]["recommender"].values()), 11)
        self.assertEqual(sum(entry["blind_high_matches"] for entry in baseline.values()), 6)
        self.assertEqual(sum(entry["unanimous_matches"] for entry in baseline.values()), 5)

    def test_an_open_earlier_reversal_resets_before_any_later_closed_match(self):
        prior = [pair(number, f"pr-{number % 4}") for number in range(1, 16)]
        prior[0]["before"]["ruling"] = "second-26"
        reversal = pair(16, decision="does-not-recover", round_name="open-review")
        reversal.update(opened="2026-10-02", closed=False)
        reversal["before"].update(ruling="second-26", group="Q1", reviews={"ruling": "second-26", "saved_outcome": "recovers"})
        reversal["before"]["answers"][1]["confidence"] = "medium"
        later = pair(17, round_name="closed-next")
        later["opened"] = "2026-10-03"
        current = pair(18, round_name="current")
        current["opened"] = "2026-10-04"
        without_later = ruling_summary.earned([*prior, reversal, current], "recovery", ROOT, "current")
        with_later = ruling_summary.earned([*prior, reversal, later, current], "recovery", ROOT, "current")
        self.assertEqual((without_later["count"], without_later["level"]), (0, 0))
        self.assertEqual((with_later["count"], with_later["level"]), (1, 0))
        self.assertEqual(without_later["resets"][0]["path"], reversal["path"])
        self.assertEqual(with_later["resets"][0]["path"], reversal["path"])
        open_match = pair(19, round_name="open-review")
        open_match.update(opened="2026-10-02", closed=False)
        self.assertEqual(ruling_summary.earned([*prior, open_match, current], "recovery", ROOT, "current")["count"], 15)

    def test_digest_sample_and_audit_calls_are_selected_by_hash(self):
        prior = [pair(number, f"pr-{number % 5}") for number in range(1, 31)]
        settled = [pair(number, round_name="second") for number in range(31, 37)]
        for item in settled:
            item["opened"] = "2026-10-02"
            item["after"].update(settled_by="agents", asked=0)
        summary = ruling_summary.summarize([*prior, *settled], round_name="second")["contract_2"]["recovery"]
        self.assertEqual(summary["round_level"], 2)
        self.assertEqual([item["ruling"] for item in summary["digest"] if item["sampled"]], ["case-31", "case-36"])
        audit = copy.deepcopy(settled)
        for item in audit:
            item["before"]["decision_type"] = "reconciliation"
        calls = ruling_summary.summarize(audit)["contract_2"]["reconciliation"]["audit_calls"]
        self.assertEqual(calls, [item["path"] for item in audit[:5]])


class PrecedentsTest(unittest.TestCase):
    def test_sheet_excludes_ruling_and_reviews_and_keeps_ground_verbatim(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            names = {"case-1": {"file": "one.md", "aliases": ["alias-1"], "reading": "current"},
                     "case-2": {"file": "two.md", "aliases": [], "reading": "current"},
                     "case-3": {"file": "three.md", "aliases": [], "reading": "earlier"},
                     "case-4": {"file": "one.md", "aliases": [], "reading": "current"}}
            (root / "index.json").write_text(json.dumps({"rulings": names}))
            for name in ("one", "two", "three"):
                (root / f"{name}.md").write_text(f"# {name} title\n")
            pairs = [pair(number) for number in range(1, 5)]
            for item in pairs:
                item["before"].update(rulings="index.json", clauses=None)
            pairs[3]["before"].update(ruling="case-4", group="Q1", reviews={"ruling": "case-1", "saved_outcome": "recovers"})
            pairs[1]["after"]["ground"] = 'My words, exactly.\nA second line with "quotes".'
            pairs[1]["after"]["lesson"] = {"says": "Read the promise.", "goes_to": "rule.md"}
            sheet = ruling_summary.precedents(pairs, "recovery", "alias-1", root)
            self.assertNotIn("case-1", sheet)
            self.assertNotIn("case-4", sheet)
            self.assertNotIn("three title", sheet)
            self.assertIn(pairs[1]["after"]["ground"], sheet)
            self.assertIn("Read the promise.", sheet)
            self.assertNotIn("gpt", sheet)
            self.assertEqual(sheet, ruling_summary.precedents(copy.deepcopy(pairs), "recovery", "alias-1", root))

    def test_real_sheet_and_cli_are_deterministic_and_do_not_overwrite(self):
        pairs = ruling_summary.records()
        sheet = ruling_summary.precedents(pairs, "recovery", "second-15")
        self.assertNotIn("Ruling: second-15.", sheet)
        self.assertIn("Ruling: second-26.", sheet)
        self.assertEqual(sheet, ruling_summary.precedents(pairs, "recovery", "second-15"))
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory, "sheet.md")
            command = [sys.executable, ruling_record.__file__, "precedents", "--type", "recovery", "--exclude", "second-15", "--out", str(output)]
            run = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stderr)
            self.assertEqual(output.read_text(), sheet)
            run = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(run.returncode, 0)
            self.assertEqual(output.read_text(), sheet)

    def test_each_answer_pins_the_sheet_it_was_given(self):
        before = sound("recovery")
        pin = before["rule"]
        before["case"]["precedents"] = pin
        self.assertTrue(all("`precedents`" in fault for fault in ruling_record.faults(before)))
        for answer in before["answers"]:
            answer["precedents"] = pin
        self.assertEqual(ruling_record.faults(before), [])
        before["answers"][1]["precedents"] = {**pin, "sha256": "0" * 64}
        self.assertIn("assessor-1: `precedents`", ruling_record.faults(before)[0])


if __name__ == "__main__":
    unittest.main()
