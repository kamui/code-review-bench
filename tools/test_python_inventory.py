"""Exercise the census against disposable tracked sources, without model calls."""
from pathlib import Path
import subprocess
import tempfile
import unittest
import json
import os
import sys

import python_inventory as inventory


class InventoryTests(unittest.TestCase):
    def test_classification_preserves_frozen_and_imported_code(self):
        cases = {
            "bench/tools/run_cell.py": "maintained",
            "bench/runs/r/source/run_cell.py": "frozen-evidence",
            "artifacts/import-source/bench/tools/run_cell.py": "imported-original",
            "bench/runs/r/inputs/skill/scripts/run.py": "target-or-skill-dependency",
            "docs/research/case/probes/probe.py": "target-or-skill-dependency",
            "docs/research/old/driver.py": "historical-replay",
            "src/lib/scoring.test.ts": "active-outside-tooling",
            "bench/harness/runner.sh": "active-outside-tooling",
            "bench/schema/run.schema.json": "active-outside-tooling",
            "bench/README.md": "active-outside-tooling",
            "bench/contributor-invocation.md": "active-outside-tooling",
            "artifacts/import-source/bench/README.md": "imported-original",
            "bench/policies/grading-commands.v1.json": "target-or-skill-dependency",
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                self.assertEqual(inventory.category(path), expected)

    def test_tracks_dynamic_paths_commands_and_embedded_interpreters(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            def write(path, text):
                destination = root / path
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_text(text)
            answers = "docs/research/builtin-review-benchmark-2026-09-24/answers.py"
            helper = "docs/research/reference-calibration-2026-10-03/blind_items.py"
            write("bench/tools/test_answers.py", f'import importlib.util\nSCRIPT = "{answers}"\nMENTION = "record.py"\n')
            write(answers, "import json\n")
            write(helper, "import json\n")
            write("docs/research/unused/record.py", "import json\n")
            write("docs/nested/workflow.md", "Run [the helper](../research/reference-calibration-2026-10-03/blind_items.py).\n")
            write("bench/tools/current_grading.py", "import json\n")
            write("tools/export_explorer.py", "import json\n")
            write("bench/tools/test_current_grading.py", "import json\n")
            write("src/smoke.test.ts", "const python = `\nimport current_grading as current, export_explorer as exporter\n"
                  "from test_current_grading import fixture\n`\nspawn(['python3', '-c', python])\n")
            write("bench/harness/runner.sh", "python3 bench/tools/current_grading.py\n")
            write("bench/schema/run.schema.json", '{"runner":"bench/tools/current_grading.py","test":"test_current_grading.py"}')
            write(".github/workflows/tests.yml", "run: python3 -m unittest discover -p test_current_grading.py\n")
            write("bench/README.md", "Run test_current_grading.py with Python.\n")
            write("bench/contributor-invocation.md", "Run current_grading.py with Python.\n")
            write("docs/research/unused/test_current_grading.py", "import json\n")
            write("bench/tools/test_last_push_move.py", "import json\n")
            activation = "docs/research/older-problem-activation-2026-10-13/filing/"
            regrade = "docs/research/regrade-rulings-2026-10-13/filing/"
            for path in (activation + "record.py", regrade + "record.py", activation + "audit.py", activation + "text.py"):
                write(path, "import json\n")
            write("bench/policies/grading-commands.v1.json", '{"test_kind":"python"}')
            write("bench/tools/regrade_queues.sh", "python3 - <<'PY'\nprint('hello')\nPY\n")
            write("package.json", '{"scripts":{"x":"python3.12 bench/tools/test_answers.py"}}')
            write("bench/import-manifest.json", '{"files":[],"repository":"fixture","revision":"unknown"}')
            write("bench/runs/frozen/manifest.json", '{"planned_cells":1,"freeze_commit":null}')
            write("bench/runs/frozen/attempts/att-001/attempt.json", '{"disposition":"harness-invalid"}')
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "add", "."], check=True)
            subprocess.run(["git", "-C", str(root), "-c", "user.name=Test", "-c", "user.email=test@example.org",
                            "commit", "-qm", "fixture"], check=True)
            write("untracked.py", "print('excluded')")
            result = inventory.census(inventory.Snapshot("HEAD", root))
            self.assertEqual(result["tracked_paths"], 26)
            files = {row["path"]: row for row in result["python_files"]}
            self.assertEqual(files[answers]["class"], "active-outside-tooling")
            self.assertEqual(files[answers]["callers"][0]["line"], 2)
            self.assertNotIn("untracked.py", files)
            self.assertEqual(files[helper]["class"], "active-outside-tooling")
            self.assertEqual(files[helper]["migration"]["issue"], 156)
            self.assertEqual(files[helper]["callers"],
                             [{"path": "docs/nested/workflow.md", "line": 1, "kind": "path-reference"}])
            self.assertEqual(files["docs/research/unused/record.py"]["class"], "historical-replay")
            self.assertEqual(files["docs/research/unused/record.py"]["callers"][0]["kind"], "possible-name-reference")
            for caller in (".github/workflows/tests.yml", "bench/schema/run.schema.json", "bench/README.md"):
                self.assertTrue(any(row["path"] == caller and row["kind"] == "possible-name-reference"
                                    for row in files["bench/tools/test_current_grading.py"]["callers"]))
            self.assertTrue(any(row["path"] == "bench/contributor-invocation.md" and row["kind"] == "possible-name-reference"
                                for row in files["bench/tools/current_grading.py"]["callers"]))
            self.assertEqual(files["docs/research/unused/test_current_grading.py"]["class"], "historical-replay")
            for directory in (activation, regrade):
                self.assertEqual(files[directory + "record.py"]["class"], "active-outside-tooling")
                self.assertTrue(any(row["kind"] == "audited-dynamic-file" for row in files[directory + "record.py"]["callers"]))
            for name in ("audit.py", "text.py"):
                self.assertEqual(files[activation + name]["class"], "historical-replay")
                self.assertNotIn("migration", files[activation + name])
            for module in ("bench/tools/current_grading.py", "tools/export_explorer.py", "bench/tools/test_current_grading.py"):
                self.assertTrue(any(row["path"] == "src/smoke.test.ts" and row["kind"] == "embedded-import-reference"
                                    for row in files[module]["callers"]))
            self.assertTrue(all(row["class"] == "active-outside-tooling" for row in result["references"]
                                if row["path"].startswith(("src/", "bench/harness/", "bench/schema/"))
                                or row["path"] in ("bench/README.md", "bench/contributor-invocation.md")))
            self.assertEqual(next(row for row in result["references"] if row["path"].startswith("bench/policies/"))["class"],
                             "target-or-skill-dependency")
            refs = {row["path"] for row in result["references"]}
            self.assertIn("bench/tools/regrade_queues.sh", refs)
            self.assertIn("package.json", refs)
            self.assertIsNone(result["runs"][0]["freeze_commit"])
            self.assertEqual(result["runs"][0]["dispositions"], {"harness-invalid": 1})

    def test_unknown_maintained_module_requires_assignment(self):
        with self.assertRaisesRegex(ValueError, "no migration owner"):
            inventory.responsibility("tools/unassigned.py")
        with self.assertRaisesRegex(ValueError, "no migration owner"):
            inventory.research_responsibility("docs/research/unassigned.py")

    def test_host_observation_preserves_unfinished_state(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            work, queues = root / "work", root / "queues"
            attempt = work / "unfinished" / "att-001"
            attempt.mkdir(parents=True)
            (attempt / "clone").mkdir()
            (attempt / "home").mkdir()
            (attempt / "home" / "credential").write_text("do not read or export")
            queue = queues / "queue"
            queue.mkdir(parents=True)
            pin = queue / "queue.pin.json"
            pin.write_text(json.dumps({"runs": {"unfinished": {"freeze_commit": "unknown"}}}))
            (queue / "controller.jsonl").write_text('{"at":"saved-time","launched":"launch-1","run":"unfinished"}\n')
            before = {str(p.relative_to(root)): p.read_bytes() for p in root.rglob("*") if p.is_file()}
            result = inventory.observe_runtime(work, queues)
            self.assertEqual(result["queues"][0]["launches_without_saved_end"], {"launch-1": "unfinished"})
            self.assertEqual(result["workspaces"][0]["attempts"][0]["present"], ["home", "clone"])
            self.assertNotIn("do not read or export", json.dumps(result))
            self.assertEqual(before, {str(p.relative_to(root)): p.read_bytes() for p in root.rglob("*") if p.is_file()})

    def test_observation_output_cannot_alias_the_census(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            script = root / "tools/python_inventory.py"
            script.parent.mkdir()
            script.write_bytes(Path(inventory.__file__).read_bytes())
            census = root / inventory.DESTINATION
            census.parent.mkdir(parents=True)
            census.write_bytes(b"retained census")
            alias = root / "alias.gz"
            alias.symlink_to(census)
            hardlink = root / "hardlink.gz"
            os.link(census, hardlink)
            command = [sys.executable, str(script), "--observe-runtime", "--work-root", str(root / "work"),
                       "--queue-root", str(root / "queues")]
            for output in ([], ["--output", str(census)], ["--output", inventory.DESTINATION],
                           ["--output", "tools/../" + inventory.DESTINATION], ["--output", str(alias)],
                           ["--output", str(hardlink)]):
                with self.subTest(output=output):
                    result = subprocess.run(command + output, cwd=root, capture_output=True, text=True)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("requires a separate --output", result.stderr)
                    self.assertEqual(census.read_bytes(), b"retained census")
            output = root / "observation.json"
            subprocess.run(command + ["--output", str(output)], cwd=root, capture_output=True, check=True)
            self.assertEqual(json.loads(output.read_text())["workspaces"], [])
            self.assertEqual(census.read_bytes(), b"retained census")


if __name__ == "__main__":
    unittest.main()
