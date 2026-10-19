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
            loaded = "docs/research/portable-evidence-2026-10-10/freeze_commits.py"
            helper = "docs/research/reference-calibration-2026-10-03/blind_items.py"
            write("bench/tools/test_loader.py", f'import importlib.util\nSCRIPT = "{loaded}"\nMENTION = "record.py"\n')
            write(loaded, "import json\n")
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
            write("package.json", '{"scripts":{"x":"python3.12 bench/tools/test_loader.py"}}')
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
            self.assertEqual(files[loaded]["class"], "active-outside-tooling")
            self.assertEqual(files[loaded]["callers"][0]["line"], 2)
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

    def test_computed_callers_resolve_siblings_and_retain_unknown_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            sources = {
                "bench/tools/dispatch.sh": '''python3 "$(dirname "$0")/clean_context.py"
python3 "$(dirname "$0")/review_isolation.py"
TOOLS=$(cd "$(dirname "$0")" && pwd)
python3 "$TOOLS/normalize_review.py"
python3 "${TOOLS}/attempt_audit.py"
python3 "$UNKNOWN/copied/grading_validation.py"
OTHER=$(dirname "$0")
OTHER=/unresolved
python3 "$OTHER/clean_context.py"
''',
                "bench/tools/grading_policy.py": '''import subprocess
from pathlib import Path
subprocess.run(["python3", str(Path(work) / "validator/tools/grading_validation.py")])
''',
                "docs/research/unused/normalize_review.py": "import json\n",
                "bench/import-manifest.json": '{"files":[],"repository":"fixture","revision":"unknown"}',
            }
            for name in ("clean_context", "review_isolation", "normalize_review", "attempt_audit", "grading_validation"):
                sources[f"bench/tools/{name}.py"] = "import json\n"
            for path, text in sources.items():
                target = root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(text)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "add", "."], check=True)
            subprocess.run(["git", "-C", str(root), "-c", "user.name=Test", "-c", "user.email=test@example.org",
                            "commit", "-qm", "fixture"], check=True)
            result = inventory.census(inventory.Snapshot("HEAD", root))
            files = {row["path"]: row for row in result["python_files"]}
            for name, line in (("clean_context", 1), ("review_isolation", 2), ("normalize_review", 4), ("attempt_audit", 5)):
                self.assertIn({"path": "bench/tools/dispatch.sh", "line": line, "kind": "path-reference"},
                              files[f"bench/tools/{name}.py"]["callers"])
            self.assertEqual(files["bench/tools/grading_validation.py"]["callers"], [
                {"path": "bench/tools/grading_policy.py", "line": 3, "kind": "possible-name-reference"},
                {"path": "bench/tools/dispatch.sh", "line": 6, "kind": "possible-name-reference"},
            ])
            self.assertIn({"path": "bench/tools/dispatch.sh", "line": 9, "kind": "possible-name-reference"},
                          files["bench/tools/clean_context.py"]["callers"])
            self.assertEqual(files["docs/research/unused/normalize_review.py"]["callers"], [])
            self.assertEqual(files["docs/research/unused/normalize_review.py"]["class"], "historical-replay")

    def test_processing_modules_belong_to_the_stage_that_removes_them(self):
        expected = {
            "bench/tools/diff_identity.py": 157, "bench/tools/compare.py": 157, "bench/tools/normalize_review.py": 158,
            "bench/tools/check_manifest.py": 160, "bench/tools/upstream.py": 156, "tools/export_explorer.py": 156,
            "bench/tools/rates.py": 160, "bench/tools/methodology.py": 156, "tools/collect_run.py": 155,
            "bench/tools/rust_runtime.py": 153, "bench/tools/seal.py": 160, "bench/tools/skill_provenance.py": 157,
            "tools/fetch_freeze_commits.py": 157, "tools/usage_pilot/adapter.py": 152,
        }
        for path, issue in expected.items():
            with self.subTest(path=path):
                self.assertEqual(inventory.responsibility(path)["issue"], issue)
        self.assertNotIn(154, {issue for issue, _ in inventory.GROUPS.values()})
        self.assertNotIn(154, inventory.RESEARCH_GROUPS)
        for name in ("roster", "derive_packet", "build_packet", "normalize_thermo", "score", "scoreboard_svg"):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "no migration owner"):
                inventory.responsibility(f"bench/tools/{name}.py")

    def test_recut_loader_keeps_only_the_staged_selection_live(self):
        recut = "docs/research/last-push-recut-2026-10-07/"
        filing = "docs/research/last-push-rulings-2026-10-11/filing/"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            sources = {
                "bench/tools/test_packet_replacements.py": "import importlib.util\n",
                "bench/tools/test_last_push_move.py": "import importlib.util\n",
                filing + "move.py": "import sys\nimport stage_edition as stage\n",
                recut + "stage_edition.py": "import json\n",
                recut + "plan.py": "import json\n",
                recut + "verify.py": "import json\n",
                "bench/import-manifest.json": '{"files":[],"repository":"fixture","revision":"unknown"}',
            }
            for path, text in sources.items():
                target = root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(text)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "add", "."], check=True)
            subprocess.run(["git", "-C", str(root), "-c", "user.name=Test", "-c", "user.email=test@example.org",
                            "commit", "-qm", "fixture"], check=True)
            files = {row["path"]: row for row in inventory.census(inventory.Snapshot("HEAD", root))["python_files"]}
            self.assertEqual(files[recut + "stage_edition.py"]["migration"]["issue"], 156)
            self.assertIn({"path": filing + "move.py", "line": 1, "kind": "audited-dynamic-file"},
                          files[recut + "stage_edition.py"]["callers"])
            for name in ("plan.py", "verify.py"):
                self.assertEqual(files[recut + name]["class"], "historical-replay")

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

    def test_observation_cli_refuses_census_options(self):
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
            for output in (["--check"], ["--ref", "HEAD"], ["--output", str(census)], ["--output", inventory.DESTINATION],
                           ["--output", "tools/../" + inventory.DESTINATION], ["--output", str(alias)],
                           ["--output", str(hardlink)]):
                with self.subTest(output=output):
                    result = subprocess.run(command + output, cwd=root, capture_output=True, text=True)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("emits JSON to stdout", result.stderr)
                    self.assertEqual(census.read_bytes(), b"retained census")
            result = subprocess.run(command, cwd=root, capture_output=True, text=True, check=True)
            self.assertEqual(json.loads(result.stdout)["workspaces"], [])
            self.assertEqual(census.read_bytes(), b"retained census")

    def test_observation_cli_leaves_runtime_paths_unchanged(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            script = root / "tools/python_inventory.py"
            script.parent.mkdir()
            script.write_bytes(Path(inventory.__file__).read_bytes())
            work, queues = root / "work", root / "queues"
            attempt = work / "unfinished" / "att-001"
            attempt.mkdir(parents=True)
            stop = attempt / "stop.json"
            stop.write_text('{"status":"unfinished"}')
            queue = queues / "queue"
            queue.mkdir(parents=True)
            pin = queue / "queue.pin.json"
            pin.write_text('{"runs":{}}')
            controller = queue / "controller.jsonl"
            controller.write_text('{"at":"saved-time","launched":"launch-1","run":"unfinished"}\n')
            alias = root / "work-alias"
            alias.symlink_to(work, target_is_directory=True)
            hardlink = root / "stop-hardlink.json"
            os.link(stop, hardlink)
            existing = root / "existing-observation.json"
            existing.write_text("retained observation")
            symlink = root / "stop-symlink.json"
            symlink.symlink_to(stop)
            command = [sys.executable, str(script), "--observe-runtime", "--work-root", str(work),
                       "--queue-root", str(queues)]
            before = {str(p.relative_to(root)): p.read_bytes() for base in (work, queues)
                      for p in base.rglob("*") if p.is_file()}
            for output in (stop, pin, controller, attempt / "new.json", queue / "new.json",
                           alias / "unfinished/att-001/new.json", hardlink, symlink, existing,
                           root / "new-observation.json",
                           root / ".local/new.json", root / ".bench/new.json"):
                with self.subTest(output=output):
                    result = subprocess.run(command + ["--output", str(output)], cwd=root,
                                            capture_output=True, text=True)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("--observe-runtime", result.stderr)
                    self.assertEqual(before, {str(p.relative_to(root)): p.read_bytes() for base in (work, queues)
                                             for p in base.rglob("*") if p.is_file()})
                    self.assertEqual(existing.read_text(), "retained observation")
            self.assertFalse((root / ".local").exists())
            self.assertFalse((root / ".bench").exists())
            self.assertFalse((root / "new-observation.json").exists())
            result = subprocess.run(command, cwd=root, capture_output=True, text=True, check=True)
            self.assertEqual(json.loads(result.stdout)["queues"][0]["launches_without_saved_end"],
                             {"launch-1": "unfinished"})
            self.assertEqual(before, {str(p.relative_to(root)): p.read_bytes() for base in (work, queues)
                                     for p in base.rglob("*") if p.is_file()})


if __name__ == "__main__":
    unittest.main()
