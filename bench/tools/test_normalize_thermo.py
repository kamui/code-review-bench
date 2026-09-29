from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

TOOL = Path(__file__).with_name("normalize_thermo.py")


class ThermoNormalizerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / "reports"
        self.root.mkdir()
        (self.root / "summary.md").write_text("## Finding\nReject the empty cache.\nThis crashes on first use.\n", encoding="utf-8")
        (self.root / "detail.md").write_text("Reject the empty cache.\nThis crashes on first use.\n", encoding="utf-8")
        self.index = {"findings": [{"report_path": "detail.md", "title": "Reject the empty cache.",
                                    "quote": "Reject the empty cache.\nThis crashes on first use.", "file": "src/cache.py",
                                    "line_start": 12, "line_end": 13}], "questions": []}
        (self.root / "finding-index.json").write_text(json.dumps(self.index), encoding="utf-8")
        self.out = Path(self.temp.name) / "normalized.json"

    def tearDown(self):
        self.temp.cleanup()

    def run_cli(self):
        return subprocess.run([sys.executable, str(TOOL), "--artifact-root", str(self.root), "--out", str(self.out)],
                              capture_output=True, text=True, check=False)

    def test_verbatim_locator_is_preserved_as_one_finding(self):
        done = self.run_cli()
        self.assertEqual(done.returncode, 0, done.stderr)
        normalized = json.loads(self.out.read_text(encoding="utf-8"))
        self.assertEqual(normalized["parse_status"], "parsed")
        self.assertEqual(len(normalized["items"]), 1)
        self.assertEqual(normalized["items"][0]["claim"], "Reject the empty cache.\nThis crashes on first use.")
        self.assertEqual(normalized["items"][0]["native_fields"]["report_path"], "detail.md")

    def test_paraphrased_quote_is_unresolved(self):
        self.index["findings"][0]["quote"] = "Reject the empty cache.\nFirst use can crash."
        (self.root / "finding-index.json").write_text(json.dumps(self.index), encoding="utf-8")
        done = self.run_cli()
        self.assertEqual(done.returncode, 1)
        normalized = json.loads(self.out.read_text(encoding="utf-8"))
        self.assertEqual(normalized["parse_status"], "unresolved")
        self.assertEqual(normalized["items"][0]["claim"], self.index["findings"][0]["quote"])

    def test_empty_requires_explicit_no_findings_in_summary(self):
        (self.root / "summary.md").write_text("Review complete.\n", encoding="utf-8")
        (self.root / "finding-index.json").write_text(json.dumps({"findings": [], "questions": []}), encoding="utf-8")
        done = self.run_cli()
        self.assertEqual(done.returncode, 1)
        self.assertEqual(json.loads(self.out.read_text(encoding="utf-8"))["parse_status"], "unresolved")
        (self.root / "summary.md").write_text("No actionable findings.\n", encoding="utf-8")
        self.assertEqual(self.run_cli().returncode, 0)
        self.assertEqual(json.loads(self.out.read_text(encoding="utf-8"))["parse_status"], "empty")

    def test_report_path_cannot_escape_root(self):
        self.index["findings"][0]["report_path"] = "../outside.md"
        (self.root / "finding-index.json").write_text(json.dumps(self.index), encoding="utf-8")
        self.assertEqual(self.run_cli().returncode, 2)


if __name__ == "__main__":
    unittest.main()
