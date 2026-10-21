from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).parent))
import rates

OPENAI = """### Standard pricing data
| Model | Short context input | Short context cached input | Short context cache writes | Short context output | Long context input | Long context cached input | Long context cache writes | Long context output |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| gpt-6.1-sol | $2.00 | $0.10 | $2.50 | $10.00 | $4.00 | $0.20 | $5.00 | $15.00 |
### Batch pricing data
| Model | Short context input | Short context cached input | Short context cache writes | Short context output | Long context input | Long context cached input | Long context cache writes | Long context output |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| gpt-6.1-sol | $1.00 | $0.05 | $1.25 | $5.00 | $2.00 | $0.10 | $2.50 | $7.50 |
"""
ANTHROPIC = """## Model pricing
| Model | Base input tokens | 5m cache writes | 1h cache writes | Cache hits and refreshes | Output tokens |
| --- | --- | --- | --- | --- | --- |
| Claude Opus 5.5 | $4 / MTok | $5 / MTok | $8 / MTok | $0.20 / MTok<sup>2</sup> | $20 / MTok |
"""
MODEL = "gpt-6.1-sol"
ROOT = Path(__file__).resolve().parent


class RatesTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.directory = Path(self.temp.name)
        self.catalog = self.directory / "rates.json"
        self.row = {"model": MODEL, "as_of": "2026-01-01", "input": 2, "output": 10,
                    "cache_read": 0.1, "cache_write_5m": 2.5, "cache_write_1h": 2.5,
                    "source": "original evidence", "billing": "list-price-equivalent"}
        self.write_catalog()
        (self.directory / "openai.md").write_text(OPENAI, encoding="utf-8")
        (self.directory / "anthropic.md").write_text(ANTHROPIC, encoding="utf-8")
        self.sources = {provider: (self.directory / f"{provider}.md").as_uri()
                        for provider in ("openai", "anthropic")}

    def tearDown(self):
        self.temp.cleanup()

    def write_catalog(self):
        self.catalog.write_text(json.dumps({"schema_version": 1, "rates": [self.row]}), encoding="utf-8")

    def command(self, action, *args):
        driver = "import sys; sys.path.insert(0, sys.argv.pop(1)); import rates; " \
                 "import json; rates.SOURCES = json.loads(sys.argv.pop(1)); sys.exit(rates.main())"
        return subprocess.run([sys.executable, "-c", driver, str(ROOT), json.dumps(self.sources),
                               action, "--rates", str(self.catalog), *args],
                              capture_output=True, text=True, encoding="utf-8")

    def test_check_and_unchanged_refresh_preserve_catalog_bytes(self):
        original = self.catalog.read_bytes()
        for action in ("check", "refresh"):
            result = self.command(action, "--json")
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            report = json.loads(result.stdout)
            self.assertEqual(report["changes"], [])
            self.assertEqual(report["prices"][MODEL]["output"], 10)
            self.assertEqual(len(report["sources"]["openai"]["sha256"]), 64)
            self.assertEqual(self.catalog.read_bytes(), original)

    def test_changed_prices_append_history_and_refresh_is_idempotent(self):
        self.row["output"] = 9
        self.write_catalog()
        original = self.catalog.read_bytes()
        result = self.command("check", "--json")
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(json.loads(result.stdout)["changes"][0]["fields"]["output"], {"saved": 9, "current": 10})
        self.assertEqual(self.catalog.read_bytes(), original)
        result = self.command("refresh")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        rows = json.loads(self.catalog.read_text(encoding="utf-8"))["rates"]
        self.assertEqual(rows[0], self.row)
        self.assertEqual(rows[1]["output"], 10)
        self.assertEqual(rows[1]["billing"], self.row["billing"])
        self.assertIn("sha256", rows[1]["source"])
        refreshed = self.catalog.read_bytes()
        self.assertEqual(self.command("refresh").returncode, 0)
        self.assertEqual(self.catalog.read_bytes(), refreshed)

    def test_source_failure_never_writes_partial_updates(self):
        self.row["output"] = 9
        self.write_catalog()
        (self.directory / "openai.md").unlink()
        original = self.catalog.read_bytes()
        result = self.command("refresh")
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("No such file", result.stderr)
        self.assertEqual(self.catalog.read_bytes(), original)

    def test_unknown_model_and_changed_provider_columns_are_refused(self):
        original = self.catalog.read_bytes()
        self.assertEqual(self.command("refresh", "--model", "gpt-unknown").returncode, 1)
        (self.directory / "openai.md").write_text(OPENAI.replace("Short context input", "Input"), encoding="utf-8")
        result = self.command("refresh")
        self.assertEqual(result.returncode, 1)
        self.assertIn("columns changed", result.stdout)
        self.assertEqual(self.catalog.read_bytes(), original)

    def test_full_catalog_run_skips_unsupported_models_and_explicit_selection_refuses_them(self):
        self.row["output"] = 9
        rows = [self.row, {**self.row, "model": "gpt-retired"}, {**self.row, "model": "gpt-unpriced"}]
        self.catalog.write_text(json.dumps({"schema_version": 1, "rates": rows}), encoding="utf-8")
        unpriced = "| gpt-unpriced | $2.00 | - | $2.50 | $9.00 | $4.00 | $0.20 | $5.00 | $15.00 |\n### Batch"
        (self.directory / "openai.md").write_text(OPENAI.replace("### Batch", unpriced, 1), encoding="utf-8")
        result = self.command("refresh", "--json")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["models"], [MODEL])
        self.assertEqual(sorted(report["unsupported"]), ["gpt-retired", "gpt-unpriced"])
        saved = json.loads(self.catalog.read_text(encoding="utf-8"))["rates"]
        self.assertEqual(saved[:3], rows)
        self.assertEqual([(row["model"], row["output"]) for row in saved[3:]], [(MODEL, 10)])
        for model in ("gpt-retired", "gpt-unpriced"):
            refused = self.command("check", "--model", MODEL, "--model", model)
            self.assertEqual(refused.returncode, 1, refused.stdout + refused.stderr)
            self.assertIn(model, refused.stdout)

    def test_provider_listing_no_cataloged_model_is_refused(self):
        self.row["model"] = "gpt-retired"
        self.write_catalog()
        original = self.catalog.read_bytes()
        result = self.command("refresh")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("gpt-retired", result.stdout)
        self.assertEqual(self.catalog.read_bytes(), original)

    def test_same_day_change_cannot_overwrite_history(self):
        self.row.update(output=9, as_of=rates.datetime.now(rates.timezone.utc).date().isoformat())
        self.write_catalog()
        original = self.catalog.read_bytes()
        result = self.command("refresh")
        self.assertEqual(result.returncode, 1)
        self.assertIn("same-day", result.stdout)
        self.assertEqual(self.catalog.read_bytes(), original)

    def test_historical_refresh_destinations_are_refused(self):
        for path in (rates.BENCH / "rates.json", rates.BENCH / "runs/test/rates.json"):
            with self.assertRaisesRegex(rates.RateError, "immutable"):
                rates.refresh(path)

    def test_anthropic_cache_prices_are_read_directly(self):
        parsed, unsupported = rates.parse_prices("anthropic", ANTHROPIC, ["claude-opus-5-5"])
        self.assertEqual(unsupported, {})
        self.assertEqual(parsed["claude-opus-5-5"], {"input": 4, "output": 20, "cache_read": 0.2,
                                                    "cache_write_5m": 5, "cache_write_1h": 8})

    def test_anthropic_model_priced_by_prompt_length_reads_its_short_prompt_row(self):
        tiered = ANTHROPIC + (
            "| Claude Haiku 5.5 (for prompts up to 100,000 tokens) | $0.10 / MTok | $0.125 / MTok | $0.20 / MTok | $0.01 / MTok | $0.50 / MTok |\n"
            "| Claude Haiku 5.5 (for prompts over 100,000 tokens) | $0.50 / MTok | $0.625 / MTok | $1 / MTok | $0.05 / MTok | $2.50 / MTok |\n")
        parsed, unsupported = rates.parse_prices("anthropic", tiered, ["claude-haiku-5-5"])
        self.assertEqual(unsupported, {})
        self.assertEqual(parsed["claude-haiku-5-5"], {"input": 0.1, "output": 0.5, "cache_read": 0.01,
                                                     "cache_write_5m": 0.125, "cache_write_1h": 0.2})

    def test_duplicate_provider_rows_are_refused(self):
        duplicated = ANTHROPIC + ANTHROPIC.splitlines()[-1] + "\n"
        with self.assertRaisesRegex(rates.RateError, "ambiguous"):
            rates.parse_prices("anthropic", duplicated, ["claude-opus-5-5"])

    def test_invalid_catalog_is_refused(self):
        self.catalog.write_text('{"schema_version": 1, "rates": [{"model": "bad"}]}', encoding="utf-8")
        result = self.command("check")
        self.assertEqual(result.returncode, 1)
        self.assertIn("missing required", result.stdout)


if __name__ == "__main__":
    unittest.main()
