#!/usr/bin/env python3
"""Drive normalize_review.py through its CLI: each arm's native output, exit codes, the timing stamp.

Usage::

    python3 bench/tools/test_normalize_review.py

The unit-level parsing cases live in ``normalize_review.py --self-test``; these tests check what
the wrapper relies on: the file written, the exit code per parse status, that ``--timing`` stamps
``payload_validated_at`` only on a parsed or empty result, and ``--render``'s blind output.

Exit codes: 0 every test passed; 1 a test failed.
"""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("normalize_review.py")


class NormalizeReviewCli(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.dir = Path(self.temp.name)
        self.out = self.dir / "normalized.json"
        self.timing = self.dir / "timing.json"
        self.timing.write_text(json.dumps({"root_dispatched_at": "2026-01-01T00:00:00Z", "payload_validated_at": None,
                                           "completed_at": None}), encoding="utf-8")

    def tearDown(self):
        self.temp.cleanup()

    def write(self, name: str, content) -> Path:
        path = self.dir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content if isinstance(content, str) else json.dumps(content), encoding="utf-8")
        return path

    def run_cli(self, *argv: str) -> subprocess.CompletedProcess:
        return subprocess.run([sys.executable, str(SCRIPT), *argv], capture_output=True, text=True, encoding="utf-8")

    def normalized(self) -> dict:
        return json.loads(self.out.read_text(encoding="utf-8"))

    def stamped(self):
        return json.loads(self.timing.read_text(encoding="utf-8"))["payload_validated_at"]

    def test_review_code_composition_parses_with_relative_paths_and_stamps_timing(self):
        composition = self.write("composition.json", {
            "summary": {"status": "Changes Requested"},
            "findings": [{"title": "Empty cart", "priority": "P1", "action": "must-fix", "trigger": "prices is []",
                          "impact": "ZeroDivisionError", "change": "return 0.0",
                          "anchor": {"path": "/clone/pricing.py", "start_line": 3, "end_line": 4}}]})
        done = self.run_cli("--arm", "review-code", "--composition", str(composition), "--clone", "/clone",
                            "--out", str(self.out), "--timing", str(self.timing))
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        doc = self.normalized()
        self.assertEqual((doc["arm"], doc["parse_status"], doc["native_verdict"]), ("review-code", "parsed", "Changes Requested"))
        self.assertEqual(doc["items"][0]["file"], "pricing.py")
        self.assertEqual(doc["items"][0]["proposed_fix"], "return 0.0")
        self.assertRegex(self.stamped() or "", r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")

    def test_review_code_without_a_status_is_unresolved_and_not_stamped(self):
        composition = self.write("composition.json", {"findings": []})
        done = self.run_cli("--arm", "review-code", "--composition", str(composition), "--out", str(self.out),
                            "--timing", str(self.timing))
        self.assertEqual(done.returncode, 1)
        self.assertIn("unresolved", done.stdout)
        self.assertEqual(self.normalized()["parse_status"], "unresolved")
        self.assertIsNone(self.stamped())

    def test_builtin_empty_array_is_an_empty_review(self):
        payload = self.write("payload.json", {"final_text": "No issues.\n```json\n[]\n```", "report_findings": []})
        done = self.run_cli("--arm", "claude-builtin", "--payload", str(payload), "--out", str(self.out),
                            "--timing", str(self.timing))
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        doc = self.normalized()
        self.assertEqual((doc["parse_status"], doc["native_verdict"], doc["items"]), ("empty", "empty-array", []))
        self.assertIsNotNone(self.stamped())

    def test_builtin_prose_without_markers_is_unresolved_and_kept_raw(self):
        payload = self.write("payload.json", {"final_text": "Looks fine to me overall.", "report_findings": []})
        done = self.run_cli("--arm", "claude-builtin", "--payload", str(payload), "--out", str(self.out),
                            "--timing", str(self.timing))
        self.assertEqual(done.returncode, 1)
        doc = self.normalized()
        self.assertEqual(doc["parse_status"], "unresolved")
        self.assertEqual(doc["items"][0]["claim"], "Looks fine to me overall.")
        self.assertIsNone(self.stamped())

    def test_builtin_fenced_json_with_an_unescaped_quote_is_unresolved_and_kept_whole(self):
        # 2026-10-08-last-push-claude-fable att-012: a finding quotes `inspect('"')` without escaping the double quote.
        def fenced(quote: str) -> str:
            return ('[\n  {\n    "file": "src/a.js",\n    "line": 58,\n    "summary": "The test no longer reaches its branch.",\n'
                    '    "failure_scenario": "A broken fallback still passes."\n  },\n'
                    '  {\n    "file": "src/b.js",\n    "line": 20,\n    "summary": "The suppression covers the whole next line.",\n'
                    f'    "failure_scenario": "A type error in the `inspect(\'{quote}\')` call is hidden."\n  }}\n]')

        def normalize(array: str) -> subprocess.CompletedProcess:
            payload = self.write("payload.json", {"final_text": f"## Review\n\n```json\n{array}\n```", "report_findings": []})
            saved = payload.read_bytes()
            done = self.run_cli("--arm", "claude-builtin", "--payload", str(payload), "--out", str(self.out),
                                "--timing", str(self.timing))
            self.assertEqual(payload.read_bytes(), saved)
            return done
        done = normalize(fenced('"'))
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        doc = self.normalized()
        self.assertEqual((doc["parse_status"], doc["native_verdict"], doc["verdict_source"]), ("unresolved", None, "final_text json"))
        self.assertEqual([item["claim"] for item in doc["items"]], [fenced('"')])
        self.assertEqual(len(doc["parse_notes"]), 1)
        self.assertIn("fenced JSON did not parse", doc["parse_notes"][0])
        self.assertIsNone(self.stamped())
        done = normalize(fenced('\\"'))
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        doc = self.normalized()
        self.assertEqual((doc["parse_status"], doc["native_verdict"]), ("parsed", "findings"))
        self.assertEqual([(item["file"], item["line_start"]) for item in doc["items"]], [("src/a.js", 58), ("src/b.js", 20)])
        self.assertEqual(doc["items"][1]["consequence"], "A type error in the `inspect('\"')` call is hidden.")
        self.assertIsNotNone(self.stamped())

    def test_builtin_report_findings_calls_from_several_workers_are_all_kept(self):
        payload = self.write("payload.json", {"final_text": "done", "report_findings": [
            {"findings": [{"file": "a.py", "line": 1, "summary": "one", "failure_scenario": "f1", "verdict": "CONFIRMED"}]},
            {"findings": [{"file": "b.py", "line": 2, "summary": "two", "failure_scenario": "f2"}]}]})
        done = self.run_cli("--arm", "claude-builtin", "--payload", str(payload), "--out", str(self.out))
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        doc = self.normalized()
        self.assertEqual([i["claim"] for i in doc["items"]], ["one", "two"])
        self.assertEqual(doc["verdict_source"], "ReportFindings")

    def test_codex_verdict_comes_from_the_rollout_when_given(self):
        stdout = self.write("stdout.txt", "The patch divides by zero.\n\nReview comment:\n\n"
                                          "- [P1] Guard empty input — /clone/pricing.py:3-4\n  Empty carts crash.\n")
        self.write("sessions/2026/01/01/rollout-child.jsonl",
                   json.dumps({"payload": {"text": '{"overall_correctness": "patch is incorrect"}'}}) + "\n")
        done = self.run_cli("--arm", "codex", "--stdout", str(stdout), "--sessions-dir", str(self.dir / "sessions"),
                            "--clone", "/clone", "--out", str(self.out))
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        doc = self.normalized()
        self.assertEqual((doc["native_verdict"], doc["verdict_source"]), ("patch is incorrect", "rollout overall_correctness"))
        self.assertEqual((doc["items"][0]["file"], doc["items"][0]["line_start"], doc["items"][0]["line_end"]), ("pricing.py", 3, 4))
        self.assertEqual(doc["items"][0]["consequence"], "Empty carts crash.")

    def test_codex_output_in_an_unknown_form_is_unresolved(self):
        stdout = self.write("stdout.txt", "Findings:\n1. something\n")
        done = self.run_cli("--arm", "codex", "--stdout", str(stdout), "--out", str(self.out))
        self.assertEqual(done.returncode, 1)
        self.assertEqual(self.normalized()["parse_status"], "unresolved")

    def test_codex_skill_preserves_native_report_and_normalizes_primary_findings_once(self):
        native = self.write("ce-review.json", {
            "status": "complete", "verdict": "Ready with fixes",
            "findings": [{"#": 1, "title": "Reject empty input", "severity": "P1", "file": "/clone/src/api.ts",
                          "line": 12, "confidence": "high", "autofix_class": "gated_auto", "owner": "downstream-resolver",
                          "suggested_fix": "Return a validation error.", "why_it_matters": "The empty input reaches an unsafe branch.",
                          "first_evidence": "if (!input)", "reviewers": ["correctness"],
                          "independent_reviewers": ["correctness"]}],
            "actionable_findings": [{"#": 1, "title": "Reject empty input"}],
            "pre_existing_findings": [{"title": "Old issue"}],
            "coverage": {"depth": "full"}, "run_id": "ce-001"})
        done = self.run_cli("--arm", "codex-skill", "--native-review", str(native), "--clone", "/clone",
                            "--out", str(self.out), "--timing", str(self.timing))
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        doc = self.normalized()
        self.assertEqual((doc["arm"], doc["native_verdict"], doc["parse_status"]),
                         ("codex-skill", "Ready with fixes", "parsed"))
        self.assertEqual(len(doc["items"]), 1)
        item = doc["items"][0]
        self.assertEqual((item["file"], item["line_start"], item["line_end"]), ("src/api.ts", 12, 12))
        self.assertEqual((item["native_priority"], item["native_action"], item["native_confidence"]),
                         ("P1", "gated_auto", "high"))
        self.assertEqual(item["native_fields"]["independent_reviewers"], ["correctness"])
        self.assertEqual(doc["native_payload"], json.loads(native.read_text(encoding="utf-8")))
        self.assertIsNotNone(self.stamped())

    def test_codex_skill_complete_empty_report_is_empty(self):
        native = self.write("ce-review.json", {"status": "complete", "verdict": "Ready to merge", "findings": [],
                                                "actionable_findings": [], "coverage": {"depth": "lite"}})
        done = self.run_cli("--arm", "codex-skill", "--native-review", str(native), "--out", str(self.out))
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertEqual(self.normalized()["parse_status"], "empty")
        self.assertEqual(self.normalized()["native_payload"]["coverage"], {"depth": "lite"})

    def test_codex_skill_incomplete_report_is_unresolved_but_keeps_items(self):
        native = self.write("ce-review.json", {"status": "degraded", "findings": [
            {"severity": "P2", "file": "src/a.py", "line": 0, "why_it_matters": "kept"}]})
        done = self.run_cli("--arm", "codex-skill", "--native-review", str(native), "--out", str(self.out))
        self.assertEqual(done.returncode, 1)
        doc = self.normalized()
        self.assertEqual(doc["parse_status"], "unresolved")
        self.assertEqual(len(doc["items"]), 1)
        self.assertTrue(any("title" in note for note in doc["parse_notes"]))
        self.assertTrue(any("line" in note for note in doc["parse_notes"]))
        self.assertEqual(doc["native_payload"], json.loads(native.read_text(encoding="utf-8")))

    def test_unreadable_input_exits_2_and_writes_nothing(self):
        payload = self.write("payload.json", "{not json")
        done = self.run_cli("--arm", "claude-builtin", "--payload", str(payload), "--out", str(self.out))
        self.assertEqual(done.returncode, 2)
        self.assertIn("normalize_review.py", done.stderr)
        self.assertFalse(self.out.exists())
        done = self.run_cli("--arm", "codex", "--stdout", str(self.dir / "missing.txt"), "--out", str(self.out))
        self.assertEqual(done.returncode, 2)

    def test_an_unreadable_timing_sidecar_exits_2(self):
        payload = self.write("payload.json", {"final_text": "```json\n[]\n```", "report_findings": []})
        self.timing.write_text("{broken", encoding="utf-8")
        done = self.run_cli("--arm", "claude-builtin", "--payload", str(payload), "--out", str(self.out),
                            "--timing", str(self.timing))
        self.assertEqual(done.returncode, 2)

    def test_a_missing_arm_input_is_a_usage_error(self):
        done = self.run_cli("--arm", "review-code", "--out", str(self.out))
        self.assertEqual(done.returncode, 2)
        self.assertIn("--composition is required", done.stderr)

    def test_render_strips_priority_and_verdict(self):
        stdout = self.write("stdout.txt", "Verdict words.\n\nReview comment:\n\n- [P0] Crash — /clone/a.py:7\n  Body.\n")
        self.run_cli("--arm", "codex", "--stdout", str(stdout), "--clone", "/clone", "--out", str(self.out))
        done = self.run_cli("--render", str(self.out))
        self.assertEqual(done.returncode, 0)
        self.assertIn("Location: a.py:7\nClaim: Crash\nConsequence: Body.", done.stdout)
        for cue in ("P0", "Verdict words", "codex"):
            self.assertNotIn(cue, done.stdout)
        empty = self.write("empty.json", {"items": []})
        self.assertIn("(no items)", self.run_cli("--render", str(empty)).stdout)


if __name__ == "__main__":
    unittest.main()
