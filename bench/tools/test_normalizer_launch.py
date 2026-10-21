"""Check that a dispatch resolves the run's pinned bench-tools before anything is claimed or a reviewer starts.

No reviewer is called: a run that pins no bench-tools, or one that predates ``skill-runner``, is refused before its claim,
and a pinned one passes the executable to the dispatch.
"""
from __future__ import annotations

import argparse
from contextlib import redirect_stderr, redirect_stdout
import hashlib
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import run_cell

FREEZE_COMMIT = "f" * 40


class Launch(unittest.TestCase):
    def setUp(self):
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory(prefix="normalizer-launch-")))
        self.run_dir = self.root / "bench/runs/2026-01-01-normalizer"
        self.run_dir.mkdir(parents=True)
        (self.run_dir / "manifest.json").write_text(json.dumps({"run_id": self.run_dir.name, "freeze_commit": FREEZE_COMMIT,
                                                                "frozen_at": "2026-01-01T00:00:00Z", "arms": []}),
                                                    encoding="utf-8")
        self.work = self.root / "work"
        self.executable = self.root / "bench-tools"
        self.executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
        self.executable.chmod(0o755)
        self.digest = hashlib.sha256(self.executable.read_bytes()).hexdigest()
        self.enterContext(patch.dict(os.environ, {"BENCH_RUST_BENCH_TOOLS": str(self.executable),
                                                  "BENCH_RUST_BENCH_TOOLS_SHA256": self.digest}))
        self.refusal = (f"run {self.run_dir.name} pins no bench-tools runtime; run it from a checkout of its freeze commit "
                        f"{FREEZE_COMMIT}")

    def attempts(self) -> list:
        """Attempt directories under the work directory, which the claim lock creates."""
        return sorted(path.name for path in self.work.glob("att-*")) if self.work.is_dir() else []

    def pin(self) -> None:
        (self.run_dir / "runtimes.frozen.json").write_text(
            json.dumps({"bench-tools": {"sha256": self.digest, "receipt_sha256": "0" * 64}}), encoding="utf-8")

    def claim(self, dry_run: bool = False):
        args = argparse.Namespace(dry_run=dry_run, quota=None)
        claim = {"cell": {"arm": "arm", "target": "fixture", "replicate": 1}, "predecessor": None}
        run = run_cell.Run(self.run_dir, self.work)
        run.arm_entry = lambda arm: {"billing_mode": "subscription"}
        run.expected_duration = lambda arm: {}
        run.manifest["caps"] = {"replacements": 0}
        with patch.object(run_cell, "Run", return_value=run), patch.object(run_cell, "check_frozen"), \
                patch.object(run_cell, "choose", return_value=claim) as choose, patch.object(run_cell, "check_target"), \
                patch.object(run_cell, "check_provenance"), \
                patch.object(run_cell, "check_dispatch_rates", return_value=({}, {"rates": []})) as rates, \
                patch.object(run_cell, "dispatch") as dispatch, \
                patch.object(run_cell, "file", return_value={"disposition": "valid completed", "usage": {"priced_total_usd": 0}}):
            try:
                result = run_cell.claim_and_run(self.run_dir, self.work, args)
            finally:
                self.calls = {"choose": choose.call_count, "rates": rates.call_count, "dispatch": dispatch.call_args_list}
        return result

    def test_a_run_that_pins_no_bench_tools_is_refused_before_its_claim_even_with_a_launch_variable(self):
        for dry_run in (True, False):
            with self.subTest(dry_run=dry_run), self.assertRaisesRegex(run_cell.Refused, self.refusal):
                self.claim(dry_run)
            self.assertEqual((self.calls["choose"], self.calls["rates"], self.calls["dispatch"]), (0, 0, []))
            self.assertEqual(self.attempts(), [])

    def test_the_cli_reports_the_refusal_with_exit_1(self):
        with patch("sys.argv", ["run_cell.py", "--run", str(self.run_dir), "--work", str(self.work), "--next"]), \
                patch.object(run_cell, "check_frozen"), redirect_stdout(io.StringIO()) as out, redirect_stderr(io.StringIO()):
            self.assertEqual(run_cell.main(), 1)
        self.assertEqual(out.getvalue(), f"refused: the bench-tools Rust runtime: {self.refusal}\n")
        self.assertEqual(self.attempts(), [])

    def test_a_pinned_run_dispatches_with_the_executable_it_pins(self):
        self.pin()
        self.claim()
        (args, _), = self.calls["dispatch"]
        self.assertEqual(args[3], {"BENCH_RUST_BENCH_TOOLS": str(self.executable), "BENCH_RUST_BENCH_TOOLS_SHA256": self.digest})
        other = self.root / "other-bench-tools"
        other.write_text("#!/bin/sh\nexit 1\n", encoding="utf-8")
        other.chmod(0o755)
        os.environ.update(BENCH_RUST_BENCH_TOOLS=str(other),
                          BENCH_RUST_BENCH_TOOLS_SHA256=hashlib.sha256(other.read_bytes()).hexdigest())
        with self.assertRaisesRegex(run_cell.Refused, f"{other} is not the bench-tools that .*runtimes.frozen.json pins"):
            self.claim()

    def test_a_pinned_bench_tools_without_skill_runner_is_refused_before_its_claim(self):
        self.executable.write_text('#!/bin/sh\n[ "$1" = skill-runner ] && exit 2\nexit 0\n', encoding="utf-8")
        self.digest = hashlib.sha256(self.executable.read_bytes()).hexdigest()
        os.environ["BENCH_RUST_BENCH_TOOLS_SHA256"] = self.digest
        self.pin()
        with self.assertRaisesRegex(run_cell.Refused, f"run {self.run_dir.name} pins a bench-tools without `skill-runner`; "
                                                      f"dispatch it from a checkout of its freeze commit {FREEZE_COMMIT}"):
            self.claim()
        self.assertEqual((self.calls["choose"], self.calls["dispatch"]), (0, []))
        self.assertEqual(self.attempts(), [])

if __name__ == "__main__":
    unittest.main()
