from __future__ import annotations

from contextlib import redirect_stdout
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[2] / "docs/research/skill-matrix-2026-10-02/dispatch_serial.py"
SPEC = importlib.util.spec_from_file_location("dispatch_serial", SCRIPT)
dispatch_serial = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dispatch_serial)


class DispatchSerialTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.run = self.root / "bench/runs/2026-10-03-test"
        self.run.mkdir(parents=True)
        (self.run / "manifest.json").write_text('{}', encoding="utf-8")
        self.work_root = self.root / "work"
        self.work = self.work_root / self.run.name
        self.work.mkdir(parents=True)
        for name, value in (("ROOT", self.root), ("WORK_ROOT", self.work_root)):
            self.enterContext(patch.object(dispatch_serial, name, value))
        self.enterContext(patch.object(dispatch_serial, "open", create=True,
                                       side_effect=lambda *args, **kwargs: self.enterContext(open(*args, **kwargs))))
        self.enterContext(patch.object(sys, "argv", [str(SCRIPT), str(self.run), "--count", "2"]))

    def file_attempt(self, *, cleaned=False):
        attempt_id = f"att-{len(list(self.work.glob('att-*'))) + 1:03d}"
        workspace = self.work / attempt_id
        workspace.mkdir()
        attempt = self.run / "attempts" / attempt_id
        attempt.mkdir(parents=True)
        record = {"disposition": "valid completed", "cell": {"target": "test"},
                  "usage": {}, "timing": {"dispatched_at": "now", "completed_at": "later"}}
        (attempt / "attempt.json").write_text(json.dumps(record), encoding="utf-8")
        if cleaned:
            (workspace / "workspace-pruned.json").write_text('{"applied": true}', encoding="utf-8")
        else:
            (workspace / "clone").mkdir()
        return workspace

    def test_nonzero_child_exit_stops_even_after_valid_filing(self):
        def child(*args, **kwargs):
            self.file_attempt()
            return subprocess.CompletedProcess(args, 2, "evidence filed", "workspace cleanup failed")

        with patch.object(dispatch_serial.subprocess, "run", side_effect=child) as launch, redirect_stdout(io.StringIO()) as output:
            self.assertEqual(dispatch_serial.main(), 1)
        self.assertEqual(launch.call_count, 1)
        result = json.loads(output.getvalue())
        self.assertEqual(result["exit"], 2)
        self.assertIn("workspace cleanup failed", result["output"])

    def test_restart_refuses_incomplete_cleanup_in_another_matrix_run(self):
        self.file_attempt()
        other = self.root / "bench/runs/2026-10-03-other"
        other.mkdir()
        with patch.object(sys, "argv", [str(SCRIPT), str(other)]), patch.object(dispatch_serial.subprocess, "run") as launch:
            with self.assertRaisesRegex(SystemExit, "workspace cleanup incomplete: 2026-10-03-test/att-001"):
                dispatch_serial.main()
        launch.assert_not_called()

    def test_receipt_does_not_override_remaining_cache(self):
        workspace = self.file_attempt(cleaned=True)
        (workspace / "clone-cache").mkdir()
        with patch.object(dispatch_serial.subprocess, "run") as launch:
            with self.assertRaisesRegex(SystemExit, "workspace cleanup incomplete"):
                dispatch_serial.main()
        launch.assert_not_called()

    def test_cleanup_receipt_is_required_even_when_clones_are_absent(self):
        workspace = self.file_attempt()
        (workspace / "clone").rmdir()
        with patch.object(dispatch_serial.subprocess, "run") as launch:
            with self.assertRaisesRegex(SystemExit, "workspace cleanup incomplete"):
                dispatch_serial.main()
        launch.assert_not_called()

    def test_successful_cleanup_allows_next_dispatch(self):
        self.file_attempt(cleaned=True)

        def child(*args, **kwargs):
            self.file_attempt(cleaned=True)
            return subprocess.CompletedProcess(args, 0, "", "")

        with patch.object(dispatch_serial.subprocess, "run", side_effect=child) as launch, redirect_stdout(io.StringIO()):
            self.assertEqual(dispatch_serial.main(), 0)
        self.assertEqual(launch.call_count, 2)


if __name__ == "__main__":
    unittest.main()
