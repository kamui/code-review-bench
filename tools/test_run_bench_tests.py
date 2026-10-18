from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from run_bench_tests import HISTORICAL, test_paths


class BenchGroupsTest(unittest.TestCase):
    def test_groups_partition_current_discovery_without_dropping_modules(self):
        directory = Path(__file__).resolve().parents[1] / "bench/tools"
        all_paths = set(test_paths(directory, "all"))
        core = set(test_paths(directory, "core"))
        historical = set(test_paths(directory, "historical"))
        expected = set(directory.glob("test_*.py")) - {directory / "test_grading_client.py"}
        self.assertEqual(all_paths, expected)
        self.assertEqual(core | historical, expected)
        self.assertFalse(core & historical)
        self.assertEqual({path.name for path in historical}, HISTORICAL)

    def test_cli_runs_requested_tests_and_defaults_to_all(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            script = root / "tools/run_bench_tests.py"
            script.parent.mkdir()
            script.write_bytes(Path(__file__).with_name("run_bench_tests.py").read_bytes())
            tests = root / "bench/tools"
            tests.mkdir(parents=True)
            fixture = "import unittest\nclass Fixture(unittest.TestCase):\n    def test_runs(self):\n        self.assertTrue(True)\n"
            for name in HISTORICAL | {"test_core.py"}:
                (tests / name).write_text(fixture)
            (tests / "test_grading_client.py").write_text("raise RuntimeError('separate client suite')\n")
            for args, expected in (([], 5), (["--group", "all"], 5), (["--group", "core"], 1),
                                   (["--group", "historical"], 4)):
                with self.subTest(args=args):
                    result = subprocess.run([sys.executable, str(script), *args], capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertIn(f"Ran {expected} test", result.stderr)
            result = subprocess.run([sys.executable, str(script), "--group", "typo"], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("invalid choice", result.stderr)


if __name__ == "__main__":
    unittest.main()
