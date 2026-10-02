"""Prove the default model list drives what is reported as benchmarked, missing, or not runnable."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

MODELS = Path(__file__).resolve().parent / "models.py"
OPUS = {"model": "claude-opus-5-5", "client": "claude-code", "effort": "high"}
LUNA = {"model": "gpt-6-luna", "client": "codex", "effort": "high"}
ASTRA = {"model": "gpt-6-astra", "client": "codex", "effort": "high"}
NOVA = {"model": "gpt-7-nova", "client": "codex", "effort": "high", "note": "released this week"}


class ModelsTest(unittest.TestCase):
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
        arms = {"builtin-opus": ("claude-builtin", "claude-opus-5-5", "high"),
                "ce-retired": ("claude-skill", "claude-sonnet-5", "high"),
                "ce-luna": ("codex-skill", "gpt-6-luna", "high"),
                "ce-astra-medium": ("codex-skill", "gpt-6-astra", "medium")}
        for arm, (kind, model, effort) in arms.items():
            self.write(f"arms/{arm}.json", {"kind": kind, "model": model, "effort": effort})
        self.suites = [
            {"id": "twelve", "entries": [self.entry("opus-builtin", "claude-builtin", "builtin-opus"),
                                         self.entry("retired-ce", "ce-code-review", "ce-retired"),
                                         self.entry("luna-ce", "ce-code-review", "ce-luna"),
                                         self.entry("astra-ce-medium", "ce-code-review", "ce-astra-medium"),
                                         {"id": "uncharted", "sources": [{"arm": "ce-astra-medium"}]}]},
            {"id": "selected", "entries": [self.entry("luna-ce", "ce-code-review", "ce-luna")]}]
        self.write("scoreboard.current.json", {"suites": self.suites})
        self.listing([OPUS, LUNA, ASTRA])

    def write(self, name, value):
        (self.bench / name).write_text(json.dumps(value), encoding="utf-8")

    def entry(self, entry_id, method, arm):
        return {"id": entry_id, "method": method, "sources": [{"arm": arm}]}

    def listing(self, models):
        self.write("models.json", {"models": models})

    def run_models(self, *arguments):
        return subprocess.run([sys.executable, str(MODELS), *arguments], capture_output=True, text=True, encoding="utf-8")

    def lines(self):
        result = self.run_models("--root", str(self.bench.parent))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return [tuple(line.split("\t")) for line in result.stdout.splitlines()]

    def test_shipped_list_is_usable(self):
        result = self.run_models()
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

    def test_one_edited_entry_adds_or_retires_a_model(self):
        before = self.lines()
        self.listing([OPUS, LUNA, ASTRA, NOVA])
        self.assertEqual([line for line in self.lines() if line not in before],
                         [("twelve", "ce-code-review", "codex", "gpt-7-nova", "high", "missing"),
                          ("selected", "ce-code-review", "codex", "gpt-7-nova", "high", "missing")])
        self.listing([OPUS, LUNA])
        self.assertEqual(self.lines(), [line for line in before if line[3] != "gpt-6-astra"])

    def test_a_method_reaches_a_client_with_its_first_benchmark_there(self):
        before = self.lines()
        self.write("arms/builtin-luna.json", {"kind": "codex", "model": "gpt-6-luna", "effort": "high"})
        self.suites[1]["entries"].append(self.entry("luna-builtin", "claude-builtin", "builtin-luna"))
        self.write("scoreboard.current.json", {"suites": self.suites})
        self.assertEqual([line for line in self.lines() if line not in before],
                         [("twelve", "claude-builtin", "codex", "gpt-6-luna", "high", "missing"),
                          ("twelve", "claude-builtin", "codex", "gpt-6-astra", "high", "missing"),
                          ("selected", "claude-builtin", "codex", "gpt-6-luna", "high", "luna-builtin"),
                          ("selected", "claude-builtin", "codex", "gpt-6-astra", "high", "missing")])

    def test_refuses_a_list_that_cannot_be_used(self):
        cases = [
            ([OPUS, {**LUNA, "model": "gpt-6-lnua"}],
             "bench/models.json: models[1]: model gpt-6-lnua has no entry in bench/rates.current.json"),
            ([{**LUNA, "client": "codecs"}], "bench/models.json: models[0]: client codecs has no bench/harness/codecs.json"),
            ([OPUS, LUNA, dict(LUNA)], "bench/models.json: models[2]: repeats gpt-6-luna at high"),
            ([OPUS, {"model": "gpt-7-nova", "client": "codex"}], "bench/models.json: models[1]: needs model, client and effort"),
            ([OPUS, "gpt-6-luna"], "bench/models.json: models[1]: needs model, client and effort"),
            ([], "bench/models.json: models must be a non-empty array")]
        for models, reason in cases:
            with self.subTest(reason=reason):
                self.listing(models)
                result = self.run_models("--root", str(self.bench.parent))
                self.assertEqual((result.returncode, result.stdout.splitlines()), (1, [reason]), result.stderr)

    def test_unreadable_input_exits_two(self):
        def exits_two(named):
            result = self.run_models("--root", str(self.bench.parent))
            self.assertEqual((result.returncode, result.stdout), (2, ""))
            self.assertIn(named, result.stderr)
            self.assertNotIn("Traceback", result.stderr)

        self.write("arms/ce-luna.json", {"kind": "gemini-skill", "model": "gpt-6-luna", "effort": "high"})
        exits_two("gemini-skill")
        (self.bench / "arms/ce-luna.json").unlink()
        exits_two("ce-luna.json")
        (self.bench / "models.json").write_text('{"models": [\n  {"model": "gpt-6-luna"}\n  {"model": "gpt-6-astra"}\n]}',
                                                encoding="utf-8")
        exits_two("models.json")
        (self.bench / "models.json").unlink()
        exits_two("models.json")


if __name__ == "__main__":
    unittest.main()
