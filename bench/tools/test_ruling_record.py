"""Prove a ruling's record is refused without its blind answers, and that the cases kept for the user are computed."""
import copy
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

import ruling_record

ROOT = Path(__file__).resolve().parents[2]
RESEARCH = ROOT / "docs/research"
REVIEW_9 = RESEARCH / "cohort-rebuild-2026-10-05/second-pass/rulings/S9-ruling-30-R2a.before.json"
SECOND = "docs/research/cohort-rebuild-2026-10-05/second-pass/"
SECOND_RULINGS = ROOT / SECOND / "rulings"
LABEL_1 = Path(__file__).with_name("test_ruling_record_fixture.json")
RULES = {"candidate": SECOND + "terms/two-questions.v6.md", "recovery": SECOND + "assessors/rule-recovery.md",
         "grouping": SECOND + "terms/two-questions.v6.md", "band": "docs/research/impact-boundary-2026-10-04/impact-boundary.v4.md",
         "control": "docs/research/reference-calibration-2026-10-03/control-audits/brief.v1.md", "reconciliation": "bench/rubric/scoring.v2.md"}
EXTRA = {"recovery": {"facts": {"says_what": "yes", "says_why": "no"}}, "grouping": {"same_fault_as": "GT-r2"}}


def pin(path):
    return {"path": path, "sha256": ruling_record.digest(ROOT / path)}


def sound(decision_type):
    """A record of this decision type, written before its question, in which every party is sure of the type's first outcome."""
    answer = {"model": "gpt-6.1-sol", "effort": "high", "family": "gpt", "blind": True, "exposure": "none",
              "brief": pin(SECOND + "assessors/prompt-candidates.md"), "written": "before-question", "outcome": ruling_record.DECISION_TYPES[decision_type]["outcomes"][0],
              "clauses": ["before-4"] if decision_type == "candidate" else [], "conflict": None, "nearest": ["second-04"], "confidence": "high",
              "short_of_high": [], "would_settle": True, "rule_gap": None, "reason": "The case file settles it.", **EXTRA.get(decision_type, {})}
    blind = [{**answer, "by": f"assessor-{number}"} for number in range(1, ruling_record.DECISION_TYPES[decision_type]["blind"] + 1)]
    return {"contract": 2, "ruling": "third-01", "decision_type": decision_type, "target": "r-base-ui-5460", "group": "N1", "reconstructed": False,
            "dossier": SECOND + "candidates/r-base-ui-5460" if decision_type == "candidate" else None,
            "case": {**pin(SECOND + "assessors/cases/r-base-ui-5460-N1.md"), "written_by": "gpt-6-astra", "precedents": None},
            "rule": pin(RULES[decision_type]), "clauses": SECOND + "terms/two-questions.v6.clauses.json" if decision_type == "candidate" else None,
            "rulings": SECOND + "terms/rulings.index.json", "reviews": None,
            "answers": [{**answer, "by": "recommender", "model": "claude-opus-5-5", "family": "claude", "blind": False, "brief": None}, *blind]}


def decided(record, directory, **changes):
    """The record saved in `directory`, and a sound after-record in which the user took the recommender's answer when first asked."""
    path = Path(directory, "third-01.before.json")
    path.write_text(json.dumps(record), encoding="utf-8")
    after = {"contract": 2, "ruling": record["ruling"], "before": {"path": str(path), "sha256": ruling_record.digest(path)}, "settled_by": "owner",
             "policy": None, "asked": 1, "outcome": record["answers"][0]["outcome"], "by_default": False, "ground": None, "miss": None, "lesson": None,
             **{field: record["answers"][0][field] for field in ("facts", "same_fault_as") if field in record["answers"][0]}}
    return path, {**after, **changes}


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
        self.record.update(decision_type="recovery", reviews=None)
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


class ContractTwoTest(unittest.TestCase):
    def refused(self, decision_type, change, *expected):
        record = sound(decision_type)
        change(record)
        self.assertEqual(ruling_record.faults(record), list(expected))

    def test_a_sound_record_of_each_decision_type_passes(self):
        for decision_type in ruling_record.DECISION_TYPES:
            with self.subTest(decision_type):
                self.assertEqual(ruling_record.faults(sound(decision_type)), [])

    def test_a_label_record_with_a_candidate_outcome_is_refused(self):
        self.refused("band", lambda record: record["answers"][1].update(outcome="problem"),
                     "assessor-1: a band outcome must be one of serious, other-material, unknown, not-applicable")
        record, after = ruling_record.load(LABEL_1), {"ruling": "second-L1", "before": pin(str(LABEL_1.relative_to(ROOT))), "asked": 1,
                                                      "outcome": "other-material", "by_default": False, "ground": None, "rule_sentence_shown": True}
        self.assertEqual(ruling_record.after_faults(record, after, LABEL_1), [])
        after["outcome"] = "problem"
        self.assertEqual(ruling_record.after_faults(record, after, LABEL_1), ["outcome must be one of serious, other-material, unknown, not-applicable"])

    def test_high_confidence_with_a_named_gap_is_refused(self):
        self.refused("candidate", lambda record: record["answers"][1].update(rule_gap="The rule does not say whose interface this is."),
                     "assessor-1: `short_of_high` names `gap` exactly when `rule_gap` says what it is")
        self.refused("candidate", lambda record: record["answers"][1].update(short_of_high=["gap"]),
                     "assessor-1: high confidence names 1 condition(s) in `short_of_high`; high has none, medium one, low two or more",
                     "assessor-1: `short_of_high` names `gap` exactly when `rule_gap` says what it is")

    def test_high_confidence_needs_a_nearest_ruling_made_under_the_current_reading(self):
        fault = "assessor-1: `nearest` names no ruling made under the current reading, so `short_of_high` must name `no-precedent`"
        self.refused("candidate", lambda record: record["answers"][1].update(nearest=[]), fault)
        self.refused("candidate", lambda record: record["answers"][1].update(nearest=["second-05", "second-pass 2", "P1"]), fault)
        self.refused("candidate", lambda record: record["answers"][1].update(nearest=["ruling 30"]), "assessor-1: ruling 30 is not a ruling in the index")
        record = sound("candidate")
        record["answers"][1].update(nearest=["second-pass 5", "S10"])
        record["answers"][2].update(nearest=["second-05"], confidence="medium", short_of_high=["no-precedent"], would_settle=False)
        self.assertEqual(ruling_record.faults(record), [])

    def test_would_settle_that_differs_from_high_is_refused(self):
        fault = "assessor-1: `would_settle` must be true at high confidence and false below it"
        self.refused("recovery", lambda record: record["answers"][1].update(would_settle=False), fault)
        self.refused("recovery", lambda record: record["answers"][1].update(confidence="medium", short_of_high=["fact-reported"]), fault)

    def test_confidence_is_one_of_three_levels_with_the_conditions_it_falls_short_on(self):
        self.refused("band", lambda record: record["answers"][1].update(confidence="medium-low", would_settle=False),
                     "assessor-1: confidence must be one of high, medium, low")
        self.refused("band", lambda record: record["answers"][1].update(confidence="low", short_of_high=["conflict"], would_settle=False),
                     "assessor-1: low confidence names 1 condition(s) in `short_of_high`; high has none, medium one, low two or more",
                     "assessor-1: `short_of_high` names `conflict` exactly when `conflict` says what it is")
        self.refused("band", lambda record: record["answers"][1].update(confidence=None),
                     "assessor-1: needs `confidence`; only a reconstructed record may leave it null, when nobody recorded it")

    def test_a_rule_that_pins_the_rubric_edited_in_place_is_refused(self):
        fault = "`rule` must pin a versioned file or the copy the round saved under docs/research, never a file that is edited in place"
        self.refused("recovery", lambda record: record.update(rule=pin("bench/rubric/scoring.md")), fault)
        self.refused("recovery", lambda record: record.update(rule=pin("docs/research/../../bench/rubric/scoring.md")), fault)

    def test_each_kind_needs_its_blind_answers_from_another_family(self):
        self.refused("band", lambda record: record["answers"].pop(), "the record needs one blind answer from another model family than the recommender's")
        self.refused("grouping", lambda record: record["answers"].pop(), "the record needs two blind answers from another model family than the recommender's")
        self.refused("control", lambda record: record["answers"][1].update(family="claude"),
                     "the record needs one blind answer from another model family than the recommender's")
        self.refused("grouping", lambda record: record["answers"][2].update(by="assessor-1"), "each party answers once; a `by` name is repeated")
        self.refused("reconciliation", lambda record: record["answers"].append({**record["answers"][0], "by": "reconciler-2"}),
                     "a reconciliation record holds one first answer, the recommender's")

    def test_an_answer_names_what_its_outcome_needs(self):
        self.refused("candidate", lambda record: record["answers"][0].update(outcome="duplicate"),
                     "recommender: `duplicate` needs `same_fault_as`, the problem it names")
        self.refused("recovery", lambda record: record["answers"][2].pop("facts"),
                     "assessor-2: a recovery answer needs `facts`: says_what and says_why, each yes, no or cannot-tell")

    def test_a_record_written_before_the_question_holds_no_later_answer(self):
        self.refused("recovery", lambda record: record["answers"][2].update(written="after-answer"),
                     "assessor-2: `written` must be one of before-question, after-answer, unknown, and before-question in a record that is not reconstructed")

    def test_the_case_and_each_blind_answers_brief_are_pinned(self):
        self.refused("band", lambda record: record.pop("case"), "the record needs `case`")
        self.refused("band", lambda record: record["case"].update(sha256="0" * 64),
                     "`case` must pin the neutral file every blind party read, by path and sha256, with `written_by` and `precedents`, "
                     "the pinned sheet of earlier rulings or null")
        for path in ("docs/ruling-dossier-brief.md", "docs/research/../ruling-dossier-brief.md"):
            self.refused("recovery", lambda record: record["answers"][2].update(brief=pin(path)),
                         "assessor-2: `brief` must pin the copy of the brief this answer was given, by path and sha256; a blind answer needs one")

    def test_a_record_decides_one_thing(self):
        fault = "`group` must name the one thing decided; a question that holds several decisions gets one record for each"
        self.refused("recovery", lambda record: record.update(group=["Q4", "Q5"]), fault)
        self.refused("candidate", lambda record: record.update(group="N1-N2"), "`group` must be one candidate of the dossier, as its summary.json names it")

    def test_a_contract_the_tool_does_not_know_is_refused(self):
        self.refused("band", lambda record: record.update(contract=3), "`contract` must be 1 or 2; a record without it is contract 1")

    def test_label_1_built_from_its_saved_files_is_accepted_and_kept_for_the_user(self):
        run = subprocess.run([sys.executable, ruling_record.__file__, LABEL_1], capture_output=True, text=True, encoding="utf-8")
        self.assertEqual(run.returncode, 0, run.stdout)
        for reason in ("a band decision is the user's under ADR-0006", "the recommendation differs from a blind assessor's answer",
                       "no rule of this decision type has been tested blind"):
            self.assertIn(f"- {reason}\n", run.stdout)

    def test_a_reconciler_call_is_not_the_users_by_decision_type(self):
        self.assertEqual(ruling_record.reasons(sound("reconciliation")), ["no rule of this decision type has been tested blind"])


class AfterContractTwoTest(unittest.TestCase):
    def refused(self, decision_type, changes, *expected):
        with tempfile.TemporaryDirectory() as directory:
            record = sound(decision_type)
            path, after = decided(record, directory, **changes)
            self.assertEqual(ruling_record.after_faults(record, after, path), list(expected))

    def test_a_sound_after_record_of_each_decision_type_passes(self):
        for decision_type in ruling_record.DECISION_TYPES:
            with self.subTest(decision_type):
                self.refused(decision_type, {})

    def test_agents_settle_nothing_while_no_policy_is_adopted(self):
        self.refused("candidate", {"settled_by": "agents", "asked": 0, "policy": pin(RULES["candidate"])},
                     "`settled_by` is `agents`, and the decision is the user's: no policy is adopted, so agents settle nothing beyond ADR-0006")
        self.refused("band", {"settled_by": "reviewer"}, "`settled_by` must be one of owner, agents")

    def test_the_policy_is_null_or_pinned(self):
        self.refused("band", {"policy": pin(RULES["band"])})
        self.refused("band", {"policy": {"path": RULES["band"], "sha256": "0" * 64}},
                     "`policy` must be null, or pin the policy the decision was made under, by path and sha256")

    def test_the_user_was_asked_at_least_once(self):
        for asked in (0, "1", True):
            self.refused("band", {"asked": asked}, "`asked` must count the times the user was asked: at least 1 when the user settled it")

    def test_the_users_answer_holds_its_plain_fields(self):
        self.refused("band", {"by_default": "no", "ground": " "}, "`by_default` must be true or false",
                     "`ground` must be the user's own words, or null when they gave none")
        self.refused("recovery", {"facts": {"says_what": "yes"}}, "a recovery decision needs `facts`: says_what and says_why, each yes, no or cannot-tell")
        self.refused("band", {"outcome": "problem", "miss": {"cause": "slip", "note": "The recommender picked the wrong label."}},
                     "outcome must be one of serious, other-material, unknown, not-applicable")

    def test_a_decision_that_names_a_known_problem_says_which(self):
        self.refused("grouping", {"same_fault_as": None}, "`same-family` needs `same_fault_as`, the problem it names")
        self.refused("candidate", {"outcome": "duplicate", "miss": {"cause": "slip", "note": "The recommender missed the known problem."}},
                     "`duplicate` needs `same_fault_as`, the problem it names")
        self.refused("candidate", {"outcome": "duplicate", "same_fault_as": "GT-r2", "miss": {"cause": "slip", "note": "The recommender missed the known problem."}})

    def test_a_miss_is_named_exactly_when_the_recommender_missed_or_the_user_was_asked_again(self):
        miss = {"cause": "fact-not-fetched", "note": "The case file did not say whose interface the setting was."}
        owed = "`miss` must name its cause: the recommender's first answer was not the decision, or the user was asked more than once"
        self.refused("band", {"outcome": "other-material"}, owed)
        self.refused("band", {"asked": 2}, owed)
        self.refused("band", {"outcome": "other-material", "miss": miss})
        self.refused("band", {"asked": 2, "miss": miss})
        self.refused("band", {"miss": miss}, "`miss` must be null: the recommender's first answer was the decision and the user was asked once")
        for wrong in ({"cause": "bad-luck", "note": "It happened."}, {"cause": "slip", "note": ""}, "slip"):
            self.refused("band", {"asked": 2, "miss": wrong}, f"`miss` must be null, or hold `cause`, one of {', '.join(ruling_record.CAUSES)}, "
                                                              "and `note`, one sentence on what happened")

    def test_a_lesson_follows_a_miss_and_names_the_file_that_changed(self):
        miss = {"cause": "not-shown", "note": "The question left out the comment's second paragraph."}
        lesson = {"says": "Show the whole comment.", "goes_to": "docs/claim-adjudication.md#prepare-a-ruling"}
        self.refused("band", {"asked": 2, "miss": miss, "lesson": lesson})
        self.refused("band", {"lesson": lesson}, "`lesson` needs a `miss`: it says what the miss changed")
        self.refused("band", {"asked": 2, "miss": miss, "lesson": {**lesson, "goes_to": "docs/no-such-page.md"}},
                     "`lesson.goes_to` must name a file in the repository, the one that was changed")
        self.refused("band", {"asked": 2, "miss": miss, "lesson": {"says": "Show the whole comment."}},
                     "`lesson` must be null, or hold `says`, one sentence, and `goes_to`, the file that was changed")

    def test_changing_the_known_problem_is_a_miss(self):
        miss = {"cause": "slip", "note": "The recommender named the wrong known problem."}
        lesson = {"says": "Check the problem identity.", "goes_to": "docs/adjudication-record.md"}
        for decision_type, outcome in (("grouping", "same-family"), ("candidate", "duplicate")):
            with self.subTest(decision_type):
                record = sound(decision_type)
                record["answers"][0].update(outcome=outcome, same_fault_as="GT-r2")
                with tempfile.TemporaryDirectory() as directory:
                    path, after = decided(record, directory)
                    self.assertEqual(ruling_record.after_faults(record, after, path), [])
                    after["same_fault_as"] = "GT-r3"
                    self.assertEqual(ruling_record.after_faults(record, after, path),
                                     ["`miss` must name its cause: the recommender's first answer was not the decision, or the user was asked more than once"])
                    after.update(miss=miss, lesson=lesson)
                    self.assertEqual(ruling_record.after_faults(record, after, path), [])

    def test_a_legacy_after_record_cannot_bypass_the_settlement_contract(self):
        legacy = ruling_record.load(str(REVIEW_9).replace(".before.", ".after."))
        for contract in (None, 1):
            with self.subTest(contract):
                after = {**legacy, "settled_by": "agents", "asked": 0}
                if contract is not None:
                    after["contract"] = contract
                record = ruling_record.load(REVIEW_9)
                faults = ruling_record.after_faults(record, after, REVIEW_9)
                self.assertTrue(any("`settled_by` is `agents`" in fault for fault in faults), faults)
        with tempfile.TemporaryDirectory() as directory:
            record = sound("candidate")
            path, after = decided(record, directory)
            after = {**legacy, "ruling": record["ruling"], "before": after["before"]}
            self.assertEqual(ruling_record.after_faults(record, after, path), [])
            for contract in (None, 1):
                with self.subTest(before_contract=2, after_contract=contract):
                    after.update(settled_by="agents", asked=0)
                    if contract is not None:
                        after["contract"] = contract
                    self.assertEqual(ruling_record.after_faults(record, after, path),
                                     ["`settled_by` is `agents`, and the decision is the user's: no policy is adopted, so agents settle nothing beyond ADR-0006"])

    def test_a_contract_2_after_record_needs_its_fields_and_a_contract_2_before_record(self):
        with tempfile.TemporaryDirectory() as directory:
            record = sound("band")
            path, after = decided(record, directory)
            self.assertEqual(ruling_record.after_faults(record, {**after, "contract": 3}, path), ["`contract` must be 1 or 2; a record without it is contract 1"])
            del after["miss"], after["settled_by"]
            self.assertEqual(ruling_record.after_faults(record, after, path), ["the after-record needs `settled_by`", "the after-record needs `miss`"])
        record, after = ruling_record.load(REVIEW_9), ruling_record.load(str(REVIEW_9).replace(".before.", ".after."))
        after.update(contract=2, settled_by="owner", policy=None, miss=None, lesson=None)
        self.assertEqual(ruling_record.after_faults(record, after, REVIEW_9), ["a contract 2 after-record belongs to a contract 2 before-record"])


class RouteTest(unittest.TestCase):
    def route(self, path):
        return subprocess.run([sys.executable, ruling_record.__file__, "route", path], capture_output=True, text=True, encoding="utf-8")

    def test_the_user_is_asked_and_shown_the_whole_case_and_every_first_answer(self):
        record = ruling_record.load(LABEL_1)
        run = self.route(LABEL_1)
        self.assertEqual(run.returncode, 0, run.stdout)
        self.assertTrue(run.stdout.startswith("ASK\n- no policy is adopted, so agents settle nothing beyond ADR-0006\n- a band decision is the user's under ADR-0006\n"))
        self.assertIn((ROOT / record["case"]["path"]).read_text(encoding="utf-8").strip(), run.stdout)
        self.assertIn("| Party | Pick | Confidence | Reason |\n| --- | --- | --- | --- |\n| recommender | other-material | low | ", run.stdout)
        self.assertEqual(run.stdout.count("\n| inspector-"), 2)

    def test_a_pick_names_the_problem_it_means_and_the_two_facts(self):
        record = sound("recovery")
        record["answers"][1].update(confidence="medium", short_of_high=["fact-reported"], would_settle=False, reason="Line one\nand | two.")
        self.assertIn("| assessor-1 | recovers (says what: yes, says why: no) | medium (fact-reported) | Line one and \\| two. |",
                      ruling_record.question(record))
        self.assertIn("| recommender | same-family of GT-r2 | high | ", ruling_record.question(sound("grouping")))

    def test_a_contract_1_record_has_no_case_file_to_show(self):
        run = self.route(REVIEW_9)
        self.assertEqual((run.returncode, run.stdout), (1, "`route` needs a contract 2 record, which pins the case file the user is shown\n"))


class RoundTest(unittest.TestCase):
    def setUp(self):
        self.directory = Path(self.enterContext(tempfile.TemporaryDirectory()), "rulings")
        shutil.copytree(SECOND_RULINGS, self.directory)

    def sheet(self, **changes):
        sheet = {**ruling_record.load(self.directory / "round.json"), **changes}
        (self.directory / "round.json").write_text(json.dumps(sheet), encoding="utf-8")

    def test_a_ruling_without_its_record_or_a_reason_is_refused(self):
        self.assertEqual(ruling_record.round_faults(self.directory), [])
        (self.directory / "11-grpc-go-Q1.before.json").unlink()
        (self.directory / "31-new-question.md").write_text("# Ruling 31\n", encoding="utf-8")
        self.assertEqual(ruling_record.round_faults(self.directory), ["11-grpc-go-Q1.md has no before-record, and `no_record` gives no reason",
                                                                     "31-new-question.md has no before-record, and `no_record` gives no reason"])
        self.sheet(no_record={"*.md": "a copy made for a test"})
        self.assertEqual(ruling_record.round_faults(self.directory), [])

    def test_a_closed_round_holds_every_answer(self):
        (self.directory / "30-base-ui-5460-Q5.N3.after.json").unlink()
        self.assertEqual(ruling_record.round_faults(self.directory), ["30-base-ui-5460-Q5.N3.before.json has no after-record, and the round is closed"])
        self.sheet(closed=False)
        self.assertEqual(ruling_record.round_faults(self.directory), [])

    def test_a_closed_round_checks_before_records_without_a_ruling_file(self):
        path, after = decided(sound("band"), self.directory)
        self.assertEqual(ruling_record.round_faults(self.directory),
                         ["third-01.before.json has no after-record, and the round is closed"])
        self.sheet(closed=False)
        self.assertEqual(ruling_record.round_faults(self.directory), [])
        self.sheet(closed=True)
        path.with_name("third-01.after.json").write_text(json.dumps(after), encoding="utf-8")
        self.assertEqual(ruling_record.round_faults(self.directory), [])

    def test_the_round_file_holds_its_fields(self):
        self.sheet(opened="5 October", no_record={"P*.md": ""})
        self.assertEqual(ruling_record.round_faults(self.directory),
                         ["round.json needs `round`, its name, `opened`, a date as YYYY-MM-DD, and `closed`, true or false",
                          "`no_record` must give, for each ruling file without a record, its name or a pattern of names and the reason"])
        (self.directory / "round.json").write_text("{}", encoding="utf-8")
        self.assertEqual(ruling_record.round_faults(self.directory), [f"round.json needs `{field}`" for field in ruling_record.ROUND])


class SavedRecordsTest(unittest.TestCase):
    def test_every_saved_record_is_sound(self):
        for path in sorted(RESEARCH.rglob("rulings/*.before.json")):
            record = ruling_record.load(path)
            self.assertEqual(ruling_record.faults(record), [], path)
            after = Path(str(path).replace(".before.", ".after."))
            if after.exists():
                self.assertEqual(ruling_record.after_faults(record, ruling_record.load(after), path), [], after)

    def test_every_round_that_holds_a_record_is_whole(self):
        rounds = sorted({path.parent for path in RESEARCH.rglob("rulings/*.before.json")})
        self.assertIn(SECOND_RULINGS, rounds)
        for directory in rounds:
            self.assertTrue((directory / "round.json").is_file(), f"{directory} holds records and no round.json")
            self.assertEqual(ruling_record.round_faults(directory), [], directory)


if __name__ == "__main__":
    unittest.main()
