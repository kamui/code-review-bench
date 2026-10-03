from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))
import rates
import run_cell

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

    def test_duplicate_provider_rows_are_refused(self):
        duplicated = ANTHROPIC + ANTHROPIC.splitlines()[-1] + "\n"
        with self.assertRaisesRegex(rates.RateError, "ambiguous"):
            rates.parse_prices("anthropic", duplicated, ["claude-opus-5-5"])

    def test_invalid_catalog_is_refused(self):
        self.catalog.write_text('{"schema_version": 1, "rates": [{"model": "bad"}]}', encoding="utf-8")
        result = self.command("check")
        self.assertEqual(result.returncode, 1)
        self.assertIn("missing required", result.stdout)

    def make_run(self):
        run_dir = self.directory / "run"
        run_dir.mkdir()
        (run_dir / "manifest.json").write_text(json.dumps({
            "arms": [{"id": "test", "billing_mode": "subscription"}],
            "rates": [{"model": MODEL, "as_of": self.row["as_of"]}]}), encoding="utf-8")
        arm_path = self.directory / "arm.json"
        arm_path.write_text(json.dumps({"model": MODEL}), encoding="utf-8")
        run = run_cell.Run(run_dir, self.directory / "work")
        run.arm_file = lambda arm: arm_path
        return run

    def test_dispatch_checks_frozen_price_even_when_latest_catalog_price_matches(self):
        self.row["output"] = 9
        self.write_catalog()
        doc = json.loads(self.catalog.read_text(encoding="utf-8"))
        doc["rates"].append({**self.row, "as_of": "2026-02-01", "output": 10})
        self.catalog.write_text(json.dumps(doc), encoding="utf-8")
        run = self.make_run()
        with patch.dict("os.environ", {"BENCH_RATES": str(self.catalog)}), patch.dict(rates.SOURCES, self.sources):
            with self.assertRaisesRegex(run_cell.Refused, "freeze a new run"):
                run_cell.check_dispatch_rates(run, {"arm": "test"})

    def test_dispatch_snapshot_matches_checked_pin(self):
        run = self.make_run()
        with patch.dict("os.environ", {"BENCH_RATES": str(self.catalog)}), patch.dict(rates.SOURCES, self.sources):
            report, snapshot = run_cell.check_dispatch_rates(run, {"arm": "test"})
        self.assertEqual(snapshot["rates"], [self.row])
        self.assertEqual(report["changes"], [])
        self.assertEqual(report["rate_pin"]["as_of"], self.row["as_of"])

    def test_failed_check_stops_before_claim_or_dispatch(self):
        run = self.make_run()
        run.manifest["caps"] = {"replacements": 0}
        run.expected_duration = lambda arm: {}
        work = self.directory / "work"
        claim = {"cell": {"arm": "test", "target": "fixture"}, "predecessor": None}
        args = argparse.Namespace(dry_run=False, quota=None)
        with patch.object(run_cell, "Run", return_value=run), patch.object(run_cell, "check_frozen"), \
                patch.object(run_cell, "choose", return_value=claim), patch.object(run_cell, "check_target"), \
                patch.object(run_cell, "check_dispatch_rates", side_effect=run_cell.Refused("price changed")), \
                patch.object(run_cell, "dispatch") as dispatch:
            with self.assertRaisesRegex(run_cell.Refused, "price changed"):
                run_cell.claim_and_run(run.dir, work, args)
        self.assertFalse((work / "att-001").exists())
        dispatch.assert_not_called()

    def test_missing_or_invalid_billing_stops_before_claim_or_price_check(self):
        run = self.make_run()
        work = self.directory / "work"
        for mode in (None, "api-dollars"):
            run.manifest["arms"][0]["billing_mode"] = mode
            for dry_run in (True, False):
                claim = {"cell": {"arm": "test", "target": "fixture"}, "predecessor": None}
                args = argparse.Namespace(dry_run=dry_run, quota=None)
                with self.subTest(mode=mode, dry_run=dry_run), \
                        patch.object(run_cell, "Run", return_value=run), patch.object(run_cell, "check_frozen"), \
                        patch.object(run_cell, "choose", return_value=claim), \
                        patch.object(run_cell, "check_dispatch_rates") as prices, \
                        patch.object(run_cell, "dispatch") as dispatch:
                    with self.assertRaisesRegex(run_cell.Refused, "declare billing_mode"):
                        run_cell.claim_and_run(run.dir, work, args)
                self.assertFalse((work / "att-001").exists())
                prices.assert_not_called()
                dispatch.assert_not_called()

    def test_successful_claim_saves_receipt_and_rate_snapshot(self):
        run = self.make_run()
        run.manifest["caps"] = {"replacements": 0}
        run.expected_duration = lambda arm: {}
        work = self.directory / "work"
        claim = {"cell": {"arm": "test", "target": "fixture"}, "predecessor": None}
        args = argparse.Namespace(dry_run=False, quota=None)
        record = {"disposition": "valid completed", "usage": {"priced_total_usd": 1}}
        with patch.object(run_cell, "Run", return_value=run), patch.object(run_cell, "check_frozen"), \
                patch.object(run_cell, "choose", return_value=claim), patch.object(run_cell, "check_target"), \
                patch.object(run_cell, "dispatch") as dispatch, patch.object(run_cell, "file", return_value=record), \
                patch.dict("os.environ", {"BENCH_RATES": str(self.catalog)}), patch.dict(rates.SOURCES, self.sources):
            result = run_cell.claim_and_run(run.dir, work, args)
        directory = work / result["attempt_id"]
        snapshot = json.loads((directory / "rates.json").read_text(encoding="utf-8"))
        receipt = json.loads((directory / "cell.json").read_text(encoding="utf-8"))["rates_check"]
        self.assertEqual(snapshot["rates"], [self.row])
        self.assertEqual(receipt["policy"], "rates-check-v1")
        self.assertEqual(json.loads((directory / "cell.json").read_text())["billing_mode"], "subscription")
        self.assertEqual(receipt["prices"][MODEL]["output"], snapshot["rates"][0]["output"])
        dispatch.assert_called_once()

    def test_filing_uses_snapshot_after_catalog_override_changes(self):
        run = self.make_run()
        attempt = "att-001"
        directory = run.work / attempt
        directory.mkdir(parents=True)
        snapshot = directory / "rates.json"
        snapshot.write_text(json.dumps({"schema_version": 1, "rates": [self.row]}), encoding="utf-8")
        (directory / "dispatch.txt").write_text("exit=0\n", encoding="utf-8")
        run.claimed[attempt] = {"cell": {"arm": "test", "target": "fixture", "replicate": 1}}
        run.arm_entry = lambda arm: {"expected_cli_version": "test"}
        run.target_dir = lambda target: self.directory
        run.arm_file("test").write_text(json.dumps({"kind": "codex", "model": MODEL}), encoding="utf-8")
        evidence = run.dir / "attempts" / attempt
        evidence.mkdir(parents=True)
        (evidence / "attempt.json").write_text(json.dumps({"disposition": "stopped"}), encoding="utf-8")
        with patch.object(run_cell, "tool", return_value=subprocess.CompletedProcess([], 0, "", "")) as tool, \
                patch.dict("os.environ", {"BENCH_RATES": "/different/catalog.json"}):
            run_cell.file(run, attempt)
        argv = tool.call_args.args[0]
        self.assertEqual(argv[argv.index("--rates") + 1], str(snapshot))
        self.assertIn("--legacy-rate-billing", argv)
        run.claimed[attempt]["billing_mode"] = "subscription"
        run.arm_entry = lambda arm: {"expected_cli_version": "test", "billing_mode": "api"}
        with patch.object(run_cell, "tool", return_value=subprocess.CompletedProcess([], 0, "", "")) as tool:
            run_cell.file(run, attempt)
        argv = tool.call_args.args[0]
        self.assertEqual(argv[argv.index("--billing-mode") + 1], "subscription")
        self.assertNotIn("--legacy-rate-billing", argv)


if __name__ == "__main__":
    unittest.main()
