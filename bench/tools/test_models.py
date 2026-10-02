"""Prove the default model list drives what is reported as benchmarked, missing, or not runnable."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

MODELS = Path(__file__).resolve().parent / "models.py"
MODEL_ROWS = ["| claude-opus-5-5 | claude-code | high |", "| gpt-6-luna | codex | high |", "| gpt-6-astra | codex | high |"]
METHOD_ROWS = ["| claude-builtin | claude-code |", "| ce-code-review | claude-code, codex |"]


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
        arms = {"builtin-opus": ("claude-opus-5-5", "high"), "ce-luna": ("gpt-6-luna", "high"),
                "ce-astra-medium": ("gpt-6-astra", "medium")}
        for arm, (model, effort) in arms.items():
            self.write(f"arms/{arm}.json", {"model": model, "effort": effort})
        self.write("scoreboard.current.json", {"suites": [
            {"id": "twelve", "entries": [self.entry("opus-builtin", "claude-builtin", "builtin-opus"),
                                         self.entry("luna-ce", "ce-code-review", "ce-luna"),
                                         self.entry("astra-ce-medium", "ce-code-review", "ce-astra-medium"),
                                         {"id": "uncharted", "sources": [{"arm": "ce-astra-medium"}]}]},
            {"id": "selected", "entries": [self.entry("luna-ce", "ce-code-review", "ce-luna")]}]})
        self.listing(MODEL_ROWS, METHOD_ROWS)

    def write(self, name, value):
        (self.bench / name).write_text(json.dumps(value), encoding="utf-8")

    def entry(self, entry_id, method, arm):
        return {"id": entry_id, "method": method, "sources": [{"arm": arm}]}

    def listing(self, models, methods, model_header="| Model | Client | Effort |"):
        lines = ["# Default models", "", "## Models", "", model_header, "| --- | --- | --- |", *models, "",
                 "## Methods", "", "| Method | Clients |", "| :-- | --: |", *methods, ""]
        (self.bench / "models.md").write_text("\n".join(lines), encoding="utf-8")

    def run_models(self, *arguments):
        return subprocess.run([sys.executable, str(MODELS), *arguments], capture_output=True, text=True, encoding="utf-8")

    def lines(self):
        result = self.run_models("--root", str(self.bench.parent))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return [tuple(line.split("\t")) for line in result.stdout.splitlines()]

    def refused(self, reason):
        result = self.run_models("--root", str(self.bench.parent))
        self.assertEqual((result.returncode, result.stdout.splitlines()), (1, [reason]), result.stderr)

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

    def test_columns_are_read_by_header_name(self):
        before = self.lines()
        reordered = ["| {2} | {0} | {1} |".format(*(cell.strip() for cell in row.strip("|").split("|"))) for row in MODEL_ROWS]
        self.listing(reordered, METHOD_ROWS, model_header="| Effort | Model | Client |")
        self.assertEqual(self.lines(), before)

    def test_one_edited_row_adds_a_model_or_lifts_a_restriction(self):
        before = self.lines()
        self.listing([*MODEL_ROWS, "| `gpt-7-nova` | codex | high | released this week |"], METHOD_ROWS,
                     model_header="| Model | Client | Effort | Notes |")
        added = [line for line in self.lines() if line not in before]
        self.assertEqual(added, [("twelve", "ce-code-review", "codex", "gpt-7-nova", "high", "missing"),
                                 ("selected", "ce-code-review", "codex", "gpt-7-nova", "high", "missing")])
        self.listing(MODEL_ROWS, ["| claude-builtin | claude-code, codex |", METHOD_ROWS[1]])
        lifted = [line for line in self.lines() if line not in before]
        self.assertEqual([line[1:4] for line in lifted if line[0] == "twelve"],
                         [("claude-builtin", "codex", "gpt-6-luna"), ("claude-builtin", "codex", "gpt-6-astra")])
        self.listing(MODEL_ROWS, [METHOD_ROWS[0], "| ce-code-review | claude-code |"])
        self.assertEqual({line[2] for line in self.lines()}, {"claude-code"})

    def test_refuses_a_list_that_cannot_be_used(self):
        cases = [
            ([*MODEL_ROWS, "| gpt-6-lnua | codex | high |"], METHOD_ROWS,
             "bench/models.md:10: model gpt-6-lnua has no entry in bench/rates.current.json"),
            (["| gpt-6-luna | codecs | high |"], METHOD_ROWS,
             "bench/models.md:7: client codecs has no bench/harness/codecs.json"),
            (MODEL_ROWS, ["| ce-code-review | claude-code, codecs |"],
             "bench/models.md:15: client codecs has no bench/harness/codecs.json"),
            ([*MODEL_ROWS, MODEL_ROWS[1]], METHOD_ROWS, "bench/models.md:10: repeats gpt-6-luna at high"),
            ([*MODEL_ROWS, "| gpt-7-nova | codex |"], METHOD_ROWS,
             "bench/models.md:10: a model row needs Model, Client and Effort"),
            (MODEL_ROWS, [*METHOD_ROWS, METHOD_ROWS[0]], "bench/models.md:17: repeats claude-builtin"),
            (MODEL_ROWS, [METHOD_ROWS[0], "| ce-code-review | |"], "bench/models.md:16: a method row needs Method and Clients"),
            (MODEL_ROWS, [], "bench/models.md: no rows under ## Methods"),
            ([], METHOD_ROWS, "bench/models.md: no rows under ## Models")]
        for models, methods, reason in cases:
            with self.subTest(reason=reason):
                self.listing(models, methods)
                self.refused(reason)

    def test_unreadable_input_exits_two(self):
        (self.bench / "arms/ce-luna.json").unlink()
        result = self.run_models("--root", str(self.bench.parent))
        self.assertEqual((result.returncode, result.stdout), (2, ""))
        self.assertIn("ce-luna.json", result.stderr)
        (self.bench / "models.md").unlink()
        self.assertEqual(self.run_models("--root", str(self.bench.parent)).returncode, 2)


if __name__ == "__main__":
    unittest.main()
