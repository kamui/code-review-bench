"""Exercise the census against disposable tracked sources, without model calls."""
from pathlib import Path
import subprocess
import tempfile
import unittest
import json

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
            write("bench/tools/test_answers.py", 'import importlib.util\nSCRIPT = "docs/research/old/answers.py"\n')
            write("docs/research/old/answers.py", "import json\n")
            write("docs/research/old/doc_helper.py", "import json\n")
            write("docs/nested/workflow.md", "Run [the helper](../research/old/doc_helper.py).\n")
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
            self.assertEqual(result["tracked_paths"], 9)
            files = {row["path"]: row for row in result["python_files"]}
            self.assertEqual(files["docs/research/old/answers.py"]["class"], "active-outside-tooling")
            self.assertEqual(files["docs/research/old/answers.py"]["callers"][0]["line"], 2)
            self.assertNotIn("untracked.py", files)
            self.assertEqual(files["docs/research/old/doc_helper.py"]["class"], "active-outside-tooling")
            self.assertEqual(files["docs/research/old/doc_helper.py"]["callers"],
                             [{"path": "docs/nested/workflow.md", "line": 1, "kind": "path-reference"}])
            refs = {row["path"] for row in result["references"]}
            self.assertIn("bench/tools/regrade_queues.sh", refs)
            self.assertIn("package.json", refs)
            self.assertIsNone(result["runs"][0]["freeze_commit"])
            self.assertEqual(result["runs"][0]["dispositions"], {"harness-invalid": 1})

    def test_unknown_maintained_module_requires_assignment(self):
        with self.assertRaisesRegex(ValueError, "no migration owner"):
            inventory.responsibility("tools/unassigned.py")

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


if __name__ == "__main__":
    unittest.main()
