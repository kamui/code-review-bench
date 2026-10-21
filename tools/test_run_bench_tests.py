from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from run_bench_tests import test_paths


class BenchTestsTest(unittest.TestCase):
    def test_every_bench_test_is_selected(self):
        directory = Path(__file__).resolve().parents[1] / "bench/tools"
        self.assertEqual(set(test_paths(directory)), set(directory.glob("test_*.py")))

    def test_cli_runs_the_selected_tests(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            script = root / "tools/run_bench_tests.py"
            script.parent.mkdir()
            script.write_bytes(Path(__file__).with_name("run_bench_tests.py").read_bytes())
            tests = root / "bench/tools"
            tests.mkdir(parents=True)
            fixture = "import unittest\nclass Fixture(unittest.TestCase):\n    def test_runs(self):\n        self.assertTrue(True)\n"
            for name in ("test_one.py", "test_two.py"):
                (tests / name).write_text(fixture)
            result = subprocess.run([sys.executable, str(script)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("Ran 2 tests", result.stderr)
            result = subprocess.run([sys.executable, str(script), "--group", "core"], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("unrecognized arguments", result.stderr)


if __name__ == "__main__":
    unittest.main()
