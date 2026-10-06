import json
import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import claude_container_resources as resources


class EffectiveLimitTests(unittest.TestCase):
    def sample(self):
        return {"limits": {"memory.max": "1073741824\n", "pids.max": "256\n",
                           "cpu.max": "200000 100000\n"}}

    def test_all_requested_limits_must_be_effective(self):
        self.assertEqual(resources.effective_limits(self.sample()), [])
        for name in resources.LIMITS:
            with self.subTest(name=name):
                sample = self.sample()
                sample["limits"][name] = None
                self.assertEqual(len(resources.effective_limits(sample)), 1)

    def test_unlimited_or_different_kernel_limits_fail(self):
        for name, value in (("memory.max", "max\n"), ("pids.max", "max\n"),
                            ("cpu.max", "max 100000\n"), ("cpu.max", "400000 100000\n"),
                            ("memory.max", "2147483648\n"), ("pids.max", "512\n")):
            with self.subTest(name=name, value=value):
                sample = self.sample()
                sample["limits"][name] = value
                self.assertEqual(len(resources.effective_limits(sample)), 1)


class ResourceProbeTests(unittest.TestCase):
    def invoke(self, root, binary, pin):
        return subprocess.run([sys.executable, resources.__file__, "--output", str(root),
                               "--image", "sha256:" + "0" * 64,
                               "--oci-runtime", str(binary), "--oci-runtime-sha256", pin],
                              capture_output=True, text=True, check=False)

    def test_runtime_pin_mismatch_retains_failure_without_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "runtime"
            binary.write_text("unexecuted runtime")
            result = self.invoke(root / "output", binary, "0" * 64)
            self.assertEqual(result.returncode, 1, result.stderr)
            receipt = json.loads((root / "output/receipt.json").read_text())
            self.assertFalse(receipt["passed"])
            self.assertFalse(receipt["issue36_complete"])
            self.assertEqual(receipt["provider_calls"], 0)
            self.assertIn("hash does not match", receipt["failures"][0])
            self.assertFalse((root / "output/run-command.json").exists())

    def test_existing_evidence_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "receipt.json").write_text("retained prior failure")
            result = self.invoke(root, root / "missing", "0" * 64)
            self.assertEqual(result.returncode, 2)
            self.assertEqual((root / "receipt.json").read_text(), "retained prior failure")

    def test_zero_exit_without_effective_cpu_is_not_admission(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            podman = root / "podman"
            sample = {"cgroup": "0::/\n", "limits": {"memory.max": "1073741824\n",
                      "pids.max": "256\n", "cpu.max": None}}
            podman.write_text("#!/usr/bin/python3\nimport json,sys\na=sys.argv[1:]\n"
                              "if a[0]=='info': print(json.dumps({'host':{'security':{'rootless':True}}}))\n"
                              "elif a[0]=='image': print(json.dumps([{'Id':'sha256:'+'0'*64}]))\n"
                              "elif 'run' in a: print(" + repr(json.dumps(sample)) + ")\n"
                              "elif a[0]=='inspect': print(json.dumps([{'State':{'Running':False}}]))\n"
                              "elif a[:2]==['container','exists']: sys.exit(1)\n")
            podman.chmod(0o755)
            runtime = Path("/usr/bin/true").resolve()
            result = subprocess.run([sys.executable, resources.__file__, "--output", str(root / "output"),
                                     "--image", "sha256:" + "0" * 64, "--oci-runtime", str(runtime),
                                     "--oci-runtime-sha256", hashlib.sha256(runtime.read_bytes()).hexdigest(),
                                     "--podman", str(podman)], capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 1, result.stderr)
            receipt = json.loads((root / "output/receipt.json").read_text())
            self.assertEqual(receipt["container_exit"], 0)
            self.assertFalse(receipt["passed"])
            self.assertIn("cpu.max", receipt["failures"][0])
            self.assertEqual(receipt["cleanup"], "removed after retained inspection")
            self.assertTrue((root / "output/inspect-stdout.txt").exists())


if __name__ == "__main__":
    unittest.main()
