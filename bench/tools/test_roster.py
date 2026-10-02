"""Prove the roster drives what is reported as benchmarked, missing, or not runnable."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROSTER = Path(__file__).resolve().parent / "roster.py"
LISTED = {"claude-code": {"claude-opus-5-5": ["high"]}, "codex": {"gpt-6-luna": ["high"], "gpt-6-astra": ["high"]}}


class RosterTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.bench = Path(temp.name) / "bench"
        for directory in ("arms", "harness"):
            (self.bench / directory).mkdir(parents=True)
        for client in ("claude-code", "codex"):
            self.write(f"harness/{client}.json", {})
        self.write("rates.current.json", {"rates": [{"model": model} for model in
                                                    ("claude-opus-5-5", "gpt-6-luna", "gpt-6-astra", "gpt-7-nova")]})
        self.arm("builtin-opus", "claude-builtin", "claude-opus-5-5")
        self.arm("ce-retired", "claude-skill", "claude-sonnet-5")
        self.arm("ce-luna", "codex-skill", "gpt-6-luna")
        self.arm("ce-astra-medium", "codex-skill", "gpt-6-astra", "medium")
        self.suites = [
            {"id": "twelve", "entries": [self.entry("opus-builtin", "claude-builtin", "builtin-opus"),
                                         self.entry("retired-ce", "ce-code-review", "ce-retired"),
                                         self.entry("luna-ce", "ce-code-review", "ce-luna"),
                                         self.entry("astra-ce-medium", "ce-code-review", "ce-astra-medium"),
                                         {"id": "uncharted", "sources": [{"arm": "ce-astra-medium"}]}]},
            {"id": "selected", "entries": [self.entry("luna-ce", "ce-code-review", "ce-luna")]}]
        self.write("scoreboard.current.json", {"suites": self.suites})
        self.write("roster.json", LISTED)

    def write(self, name, value):
        (self.bench / name).write_text(json.dumps(value), encoding="utf-8")

    def arm(self, arm, kind, model, effort="high"):
        self.write(f"arms/{arm}.json", {"kind": kind, "model": model, "effort": effort})

    def entry(self, entry_id, method, arm):
        return {"id": entry_id, "method": method, "sources": [{"arm": arm}]}

    def run_roster(self, *arguments):
        return subprocess.run([sys.executable, str(ROSTER), *arguments], capture_output=True, text=True, encoding="utf-8")

    def lines(self):
        result = self.run_roster("--root", str(self.bench.parent))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return [tuple(line.split("\t")) for line in result.stdout.splitlines()]

    def added(self, before):
        return [line for line in self.lines() if line not in before]

    def test_shipped_roster_is_usable(self):
        result = self.run_roster()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertTrue(result.stdout)

    def test_reports_each_runnable_combination_as_benchmarked_or_missing(self):
        self.assertEqual(self.lines(), [
            ("twelve", "claude-builtin", "claude-code", "claude-opus-5-5", "high", "opus-builtin"),
            ("twelve", "ce-code-review", "claude-code", "claude-opus-5-5", "high", "missing"),
            ("twelve", "ce-code-review", "codex", "gpt-6-luna", "high", "luna-ce"),
            ("twelve", "ce-code-review", "codex", "gpt-6-astra", "high", "missing"),
            ("selected", "claude-builtin", "claude-code", "claude-opus-5-5", "high", "missing"),
            ("selected", "ce-code-review", "claude-code", "claude-opus-5-5", "high", "missing"),
            ("selected", "ce-code-review", "codex", "gpt-6-luna", "high", "luna-ce"),
            ("selected", "ce-code-review", "codex", "gpt-6-astra", "high", "missing")])

    def test_one_edited_line_adds_a_model_an_effort_or_retires_a_model(self):
        before = self.lines()
        self.write("roster.json", {**LISTED, "codex": {**LISTED["codex"], "gpt-7-nova": ["high"]}})
        self.assertEqual(self.added(before), [("twelve", "ce-code-review", "codex", "gpt-7-nova", "high", "missing"),
                                              ("selected", "ce-code-review", "codex", "gpt-7-nova", "high", "missing")])
        self.write("roster.json", {**LISTED, "codex": {**LISTED["codex"], "gpt-6-astra": ["high", "medium"]}})
        self.assertEqual(self.added(before), [("twelve", "ce-code-review", "codex", "gpt-6-astra", "medium", "astra-ce-medium"),
                                              ("selected", "ce-code-review", "codex", "gpt-6-astra", "medium", "missing")])
        self.write("roster.json", {**LISTED, "codex": {"gpt-6-luna": ["high"]}})
        self.assertEqual(self.lines(), [line for line in before if line[3] != "gpt-6-astra"])

    def test_a_method_reaches_a_client_with_its_first_benchmark_there(self):
        before = self.lines()
        self.arm("builtin-luna", "codex", "gpt-6-luna")
        self.suites[1]["entries"].append(self.entry("luna-builtin", "claude-builtin", "builtin-luna"))
        self.write("scoreboard.current.json", {"suites": self.suites})
        self.assertEqual(self.added(before), [("twelve", "claude-builtin", "codex", "gpt-6-luna", "high", "missing"),
                                              ("twelve", "claude-builtin", "codex", "gpt-6-astra", "high", "missing"),
                                              ("selected", "claude-builtin", "codex", "gpt-6-luna", "high", "luna-builtin"),
                                              ("selected", "claude-builtin", "codex", "gpt-6-astra", "high", "missing")])

    def test_a_benchmark_on_one_client_does_not_cover_the_model_on_another(self):
        before = self.lines()
        self.write("roster.json", {**LISTED, "codex": {**LISTED["codex"], "claude-opus-5-5": ["high"]}})
        self.arm("ce-opus", "claude-skill", "claude-opus-5-5")
        self.suites[0]["entries"].append(self.entry("opus-ce", "ce-code-review", "ce-opus"))
        self.write("scoreboard.current.json", {"suites": self.suites})
        self.assertEqual(self.added(before), [("twelve", "ce-code-review", "claude-code", "claude-opus-5-5", "high", "opus-ce"),
                                              ("twelve", "ce-code-review", "codex", "claude-opus-5-5", "high", "missing"),
                                              ("selected", "ce-code-review", "codex", "claude-opus-5-5", "high", "missing")])

    def test_refuses_a_roster_that_cannot_be_used(self):
        cases = [
            ({"codex": {"gpt-6-lnua": ["high"]}}, "bench/roster.json: model gpt-6-lnua has no entry in bench/rates.current.json"),
            ({"codecs": {"gpt-6-luna": ["high"]}}, "bench/roster.json: client codecs has no bench/harness/codecs.json"),
            ({"codex": ["gpt-6-luna"]}, "bench/roster.json: codex needs an object of models"),
            ({"codex": {"gpt-6-luna": "high"}}, "bench/roster.json: codex gpt-6-luna needs a list of distinct efforts"),
            ({"codex": {"gpt-6-luna": []}}, "bench/roster.json: codex gpt-6-luna needs a list of distinct efforts"),
            ({"codex": {"gpt-6-luna": ["high", 3]}}, "bench/roster.json: codex gpt-6-luna needs a list of distinct efforts"),
            ({"codex": {"gpt-6-luna": ["high", "high"]}}, "bench/roster.json: codex gpt-6-luna needs a list of distinct efforts"),
            (["gpt-6-luna"], "bench/roster.json: needs an object keyed by client"),
            ({"codex": {}}, "bench/roster.json: lists no models")]
        for listed, reason in cases:
            with self.subTest(reason=reason):
                self.write("roster.json", listed)
                result = self.run_roster("--root", str(self.bench.parent))
                self.assertEqual((result.returncode, result.stdout.splitlines()), (1, [reason]), result.stderr)

    def test_unreadable_input_exits_two(self):
        def exits_two(named):
            result = self.run_roster("--root", str(self.bench.parent))
            self.assertEqual((result.returncode, result.stdout), (2, ""))
            self.assertIn(named, result.stderr)
            self.assertNotIn("Traceback", result.stderr)

        self.arm("ce-luna", "gemini-skill", "gpt-6-luna")
        exits_two("gemini-skill")
        (self.bench / "arms/ce-luna.json").unlink()
        exits_two("ce-luna.json")
        listed = self.bench / "roster.json"
        listed.write_text('{"codex": {"gpt-6-luna": ["high"], "gpt-6-astra": ["high"], "gpt-6-luna": ["medium"]}}', encoding="utf-8")
        exits_two("gpt-6-luna is written more than once")
        listed.write_text('{"codex": {"gpt-6-luna": ["high"]\n "gpt-6-astra": ["high"]}}', encoding="utf-8")
        exits_two("roster.json")
        listed.unlink()
        exits_two("roster.json")


if __name__ == "__main__":
    unittest.main()
