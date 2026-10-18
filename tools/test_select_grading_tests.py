import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from select_grading_tests import selection


SCRIPT = Path(__file__).with_name("select_grading_tests.py")


class GradingSelectionTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q")
        self.git("config", "user.email", "test@example.org")
        self.git("config", "user.name", "Test")
        self.write("src/components/Chart.tsx", "initial UI")
        self.write("bench/tools/grade.py", "initial grading")
        self.base = self.commit()

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, text=True).strip()

    def write(self, name, content="changed"):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)

    def commit(self):
        self.git("add", "-A")
        self.git("commit", "-qm", "fixture")
        return self.git("rev-parse", "HEAD")

    def event(self):
        return {"pull_request": {"base": {"sha": self.base}, "head": {"sha": self.git("rev-parse", "HEAD")}}}

    def test_ui_and_assets_skip(self):
        self.write("src/components/Chart.tsx")
        self.write("src/routes/index.tsx")
        self.write("src/styles.css")
        self.write("public/icon.svg")
        self.write("src/components/newline\nchart.tsx")
        self.commit()
        self.assertFalse(selection("pull_request", self.event(), self.root)[0])

    def test_relevant_and_unknown_paths_run(self):
        for path in ("bench/tools/grade.py", "tools/export_explorer.py", "docs/research/input.md",
                     "docs/clean-context.md", "README.md", "package.json", "bun.lock",
                     ".github/workflows/pages.yml", "src/lib/scoring.ts", "new-file",
                     "src/components/unknown.py", "public/client-pin.json"):
            with self.subTest(path=path):
                self.write(path, path)
                self.commit()
                self.assertTrue(selection("pull_request", self.event(), self.root)[0])
                self.base = self.git("rev-parse", "HEAD")

    def test_earlier_pr_commit_is_not_hidden_by_latest_ui_commit(self):
        self.write("bench/tools/grade.py")
        self.commit()
        self.write("src/components/Chart.tsx")
        self.commit()
        self.assertTrue(selection("pull_request", self.event(), self.root)[0])

    def test_rename_from_relevant_path_runs(self):
        (self.root / "public").mkdir()
        self.git("mv", "bench/tools/grade.py", "public/icon.svg")
        self.commit()
        self.assertTrue(selection("pull_request", self.event(), self.root)[0])

    def test_rename_to_relevant_path_runs(self):
        self.git("mv", "src/components/Chart.tsx", "bench/tools/other.py")
        self.commit()
        self.assertTrue(selection("pull_request", self.event(), self.root)[0])

    def test_deleted_relevant_path_runs(self):
        self.git("rm", "bench/tools/grade.py")
        self.commit()
        self.assertTrue(selection("pull_request", self.event(), self.root)[0])

    def test_deleted_ui_path_skips(self):
        self.git("rm", "src/components/Chart.tsx")
        self.commit()
        self.assertFalse(selection("pull_request", self.event(), self.root)[0])

    def test_main_and_manual_runs_do_not_depend_on_diff(self):
        for event in ("push", "workflow_dispatch"):
            self.assertTrue(selection(event, {}, self.root)[0])

    def test_empty_diff_runs(self):
        self.assertTrue(selection("pull_request", self.event(), self.root)[0])

    def test_cli_detection_failures_default_to_full_coverage(self):
        event_path, output = self.root / "event.json", self.root / "output"
        for event in ("invalid JSON", "{}", json.dumps({"pull_request": {
                "base": {"sha": "missing-base"}, "head": {"sha": "missing-head"}}})):
            with self.subTest(event=event):
                event_path.write_text(event)
                output.write_text("")
                result = subprocess.run([sys.executable, str(SCRIPT), "--event-name", "pull_request",
                                         "--event-path", str(event_path), "--output", str(output)],
                                        capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(output.read_text(), "run=true\n")
                self.assertIn("Change detection unavailable", result.stdout)


if __name__ == "__main__":
    unittest.main()
