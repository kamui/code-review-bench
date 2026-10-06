import json
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import claude_container_controller_v4 as controller


class CleanupEvidenceTests(unittest.TestCase):
    def test_repeat_cleanup_preserves_inspection_captured_before_removal(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            evidence = output / "container-inspect.json"
            captured = '[{"State":{"Running":false},"HostConfig":{"Memory":1073741824}}]'
            evidence.write_text(captured)
            absent = subprocess.CompletedProcess([], 125, "", "no such container")
            proven_absent = subprocess.CompletedProcess([], 1, "", "")
            with patch.object(controller.legacy, "run", side_effect=[absent, proven_absent]):
                result = controller.cleanup(SimpleNamespace(podman="podman"), "owned-fixture", output)
            self.assertTrue(result["already_absent"])
            self.assertEqual(evidence.read_text(), captured)

    def test_inspect_errors_require_proven_absence(self):
        for exists_code in (0, 125):
            with self.subTest(exists_code=exists_code), tempfile.TemporaryDirectory() as directory:
                inspect_error = subprocess.CompletedProcess([], 125, "", "permission denied")
                query = subprocess.CompletedProcess([], exists_code, "", "")
                with patch.object(controller.legacy, "run", side_effect=[inspect_error, query]):
                    with self.assertRaises(controller.legacy.ProbeError):
                        controller.cleanup(SimpleNamespace(podman="podman"), "owned-fixture", Path(directory))

    def test_failed_removal_cannot_be_recorded_as_completed_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            inspected = subprocess.CompletedProcess([], 0, json.dumps([{"State": {"Running": False}}]), "")
            failed = subprocess.CompletedProcess([], 1, "", "removal refused")
            remains = subprocess.CompletedProcess([], 0, "", "")
            with patch.object(controller.legacy, "run", side_effect=[inspected, failed, remains]):
                with self.assertRaises(controller.legacy.ProbeError):
                    controller.cleanup(SimpleNamespace(podman="podman"), "owned-fixture", Path(directory))


if __name__ == "__main__":
    unittest.main()
