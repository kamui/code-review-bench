import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from select_grading_tests import JOBS, selection


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
        self.write("rust/bench-tools/src/lib.rs", "initial runtime")
        self.write("rust/bench-tools/tests/reader.rs",
                   'root.join("docs/read.md"); root.join("docs/data"); root.join("docs/slash/"); "docs"')
        self.write("README.md", "Prose that links [unread](docs/unread.md) reads nothing.")
        self.write("docs/unread.md", "initial prose")
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

    def selected(self, *paths):
        for path in paths:
            self.write(path, f"changed after {self.base}")
        self.commit()
        jobs = selection("pull_request", self.event(), self.root)[0]
        self.base = self.git("rev-parse", "HEAD")
        return jobs

    def test_ui_and_assets_skip(self):
        self.assertEqual(self.selected("src/components/Chart.tsx", "src/routes/index.tsx", "src/styles.css",
                                       "public/icon.svg", "src/components/newline\nchart.tsx",
                                       ".github/workflows/rust-runtime.yml"), set())

    def test_rust_runtime_workflow_alone_skips(self):
        self.assertEqual(self.selected(".github/workflows/rust-runtime.yml"), set())

    def test_each_job_skips_for_a_path_it_does_not_read(self):
        for paths, jobs in ((("tools/test_export.py",), {"build"}),
                            (("tools/run_bench_tests.py",), {"build", "grading-tests"}),
                            (("tools/fetch_freeze_commits.py",), {"build", "grading-tests"}),
                            (("bench/tools/grade.py", "bench/tools/test_grade.py"), {"grading-tests"}),
                            (("tools/test_export.py", "bench/tools/grade.py", "src/routes/index.tsx"),
                             {"build", "grading-tests"})):
            with self.subTest(paths=paths):
                self.assertEqual(self.selected(*paths), jobs)

    def test_relevant_and_unknown_paths_run_every_job(self):
        for path in ("rust/bench-tools/src/lib.rs", "bench/tools/rust_runtime.py", "tools/select_grading_tests.py",
                     "tools/export_explorer.py", "tools/behavior_contracts/grade-map.v1.json",
                     "tools/test_data.json", "bench/tools/dispatch.sh", "bench/tools/sub/grade.py",
                     "bench/grading/current/assessments/receipt.json", "bench/regrading/evidence.json",
                     "package.json", "bun.lock", ".github/workflows/pages.yml", ".github/workflows/rates.yml",
                     "src/lib/scoring.ts", "new-file", "src/components/unknown.py", "public/client-pin.json"):
            with self.subTest(path=path):
                self.assertEqual(self.selected(path), JOBS)

    def test_doc_no_code_reads_skips(self):
        for paths in (("docs/unread.md",), ("docs/new/guide.md", "NOTES.md"), ("docs/read.md.orig",),
                      ("docs/data-v2/table.md",), ("docs/other.json",),
                      ("docs/unread.md", "src/components/Chart.tsx")):
            with self.subTest(paths=paths):
                self.assertEqual(self.selected(*paths), set())

    def test_doc_code_reads_runs_every_job(self):
        for paths in (("docs/read.md",), ("docs/data/table.md",), ("docs/slash/deeper/table.md",),
                      ("docs/unread.md", "docs/read.md"), ("docs/read.md", "bench/tools/grade.py")):
            with self.subTest(paths=paths):
                self.assertEqual(self.selected(*paths), JOBS)

    def test_doc_that_names_only_itself_skips(self):
        self.write("docs/rollback.sh", "# Usage: docs/rollback.sh SCRATCH")
        self.commit()
        self.assertEqual(selection("pull_request", self.event(), self.root)[0], set())

    def test_doc_alongside_a_tool_selects_only_the_tool_jobs(self):
        self.assertEqual(self.selected("docs/unread.md", "bench/tools/grade.py"), {"grading-tests"})

    def test_doc_named_by_a_fixture_or_record_runs_every_job(self):
        self.write("bench/fixtures/case.json", '{"path": "docs/unread.md"}')
        self.commit()
        self.base = self.git("rev-parse", "HEAD")
        self.assertEqual(self.selected("docs/unread.md"), JOBS)

    def test_deleted_doc_code_still_names_runs(self):
        self.write("docs/read.md")
        self.base = self.commit()
        self.git("rm", "-q", "docs/read.md")
        self.commit()
        self.assertEqual(selection("pull_request", self.event(), self.root)[0], JOBS)

    def test_earlier_pr_commit_is_not_hidden_by_latest_ui_commit(self):
        self.write("rust/bench-tools/src/lib.rs")
        self.commit()
        self.write("src/components/Chart.tsx")
        self.commit()
        self.assertEqual(selection("pull_request", self.event(), self.root)[0], JOBS)

    def test_rename_from_relevant_path_runs(self):
        (self.root / "public").mkdir()
        self.git("mv", "rust/bench-tools/src/lib.rs", "public/icon.svg")
        self.commit()
        self.assertEqual(selection("pull_request", self.event(), self.root)[0], JOBS)

    def test_rename_to_relevant_path_runs(self):
        self.git("mv", "src/components/Chart.tsx", "rust/bench-tools/src/chart.rs")
        self.commit()
        self.assertEqual(selection("pull_request", self.event(), self.root)[0], JOBS)

    def test_deleted_relevant_path_runs(self):
        self.git("rm", "-q", "rust/bench-tools/src/lib.rs")
        self.commit()
        self.assertEqual(selection("pull_request", self.event(), self.root)[0], JOBS)

    def test_deleted_ui_path_skips(self):
        self.git("rm", "-q", "src/components/Chart.tsx")
        self.commit()
        self.assertEqual(selection("pull_request", self.event(), self.root)[0], set())

    def test_main_and_manual_runs_do_not_depend_on_diff(self):
        for event in ("push", "workflow_dispatch"):
            self.assertEqual(selection(event, {}, self.root)[0], JOBS)

    def test_empty_diff_runs(self):
        self.assertEqual(selection("pull_request", self.event(), self.root)[0], JOBS)

    def run_cli(self, event, job):
        script, event_path, output = self.root / "tools/select_grading_tests.py", self.root / "event.json", self.root / "output"
        script.parent.mkdir(exist_ok=True)
        script.write_bytes(SCRIPT.read_bytes())
        event_path.write_text(event)
        output.write_text("")
        result = subprocess.run([sys.executable, str(script), "--event-name", "pull_request",
                                 "--event-path", str(event_path), "--job", job, "--output", str(output)],
                                capture_output=True, text=True)
        return result, output.read_text()

    def test_cli_writes_each_job_selection(self):
        self.write("tools/test_export.py")
        self.commit()
        event = json.dumps(self.event())
        for job in sorted(JOBS):
            with self.subTest(job=job):
                result, output = self.run_cli(event, job)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(output, "run=true\n" if job == "build" else "run=false\n")

    def test_cli_detection_failures_default_to_full_coverage(self):
        for event in ("invalid JSON", "{}", json.dumps({"pull_request": {
                "base": {"sha": "missing-base"}, "head": {"sha": "missing-head"}}})):
            with self.subTest(event=event):
                result, output = self.run_cli(event, "mapping-replay")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(output, "run=true\n")
                self.assertIn("Change detection unavailable", result.stdout)


if __name__ == "__main__":
    unittest.main()
