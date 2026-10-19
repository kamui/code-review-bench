"""Fail when a current caller still runs a command whose implementation moved to bench-tools."""
from __future__ import annotations

import json
from pathlib import Path
import re
import subprocess
import unittest

REPO = Path(__file__).resolve().parents[1]
# Retained library, the command that moved out of it, and the command that replaces it.
MOVED = (
    ("bench/tools/rates.py", "check", "bench-tools rates check"),
    ("bench/tools/upstream.py", "report", "bench-tools upstream report"),
    ("bench/tools/evidence_store.py", "fetch", "bench-tools evidence fetch"),
    ("bench/tools/evidence_store.py", "verify", "bench-tools evidence verify"),
    ("bench/tools/recover_evidence.py", "--source", "bench-tools evidence recover"),
    ("bench/tools/evidence_inventory.py", "--root", "bench-tools evidence inventory"),
    ("tools/collect_run.py", "--run", "bench-tools evidence collect"),
    ("bench/tools/reconcile_evidence.py", "--inventory", "bench-tools evidence reconcile"),
    ("bench/tools/ruling_record.py", "route", "bench-tools ruling route"),
    ("bench/tools/ruling_record.py", "summary", "bench-tools ruling summary"),
    ("bench/tools/ruling_record.py", "precedents", "bench-tools ruling precedents"),
)
# Retained libraries whose command line moved whole, including the form without a command word.
MOVED_PROGRAMS = (
    ("bench/tools/ruling_record.py", "bench-tools ruling check"),
    ("bench/tools/ruling_dossier.py", "bench-tools ruling dossier"),
)
# Frozen runs, imported originals, research records, saved rulings, saved fixture and migration evidence, and the
# contract decisions that describe superseded behavior keep the commands they recorded.
HISTORICAL = ("artifacts/", "bench/runs/", "docs/research/", "bench/grading/rulings/", "bench/fixtures/",
              "docs/python-migration/evidence/", "docs/python-migration/pilot/", "tools/behavior_contracts/")


def invocation(module, command):
    """A shell command, a quoted argument list or prose that runs `module command`."""
    return re.compile(rf"\b{re.escape(Path(module).name)}[\"')\],\s`]+{re.escape(command)}\b")


def program_run(module):
    """`python3 path/module ...`, or a quoted command line that starts with the module and has an argument."""
    name = re.escape(Path(module).name)
    return re.compile(rf"\bpython3?\s+(?:\S*/)?{name}\b|`{name}\s+[^`]+`")


def current_text_files():
    names = subprocess.run(["git", "-C", str(REPO), "ls-files", "-z"], capture_output=True, check=True).stdout
    for name in names.decode("utf-8", "surrogateescape").split("\0"):
        if not name or name.startswith(HISTORICAL) or name == "tools/test_moved_cli_callers.py":
            continue
        path = REPO / name
        if not path.is_file() or path.is_symlink():
            continue
        try:
            yield name, path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue


class MovedCommands(unittest.TestCase):
    def test_the_pattern_finds_each_calling_form(self):
        pattern = invocation("bench/tools/rates.py", "check")
        for text in ('"rates:check": "python3 bench/tools/rates.py check"', "invoke `rates.py check` first",
                     '[sys.executable, str(TOOLS / "rates.py"), "check", "--json"]', "rates.py\tcheck"):
            self.assertRegex(text, pattern)
        for text in ("python3 bench/tools/rates.py refresh", "`rates.py`'s `check` command", "pirates.py check"):
            self.assertNotRegex(text, pattern)

    def test_the_program_pattern_finds_a_run_without_a_command_word(self):
        pattern = program_run("bench/tools/ruling_dossier.py")
        for text in ("run `python3 bench/tools/ruling_dossier.py {OUT}`", "`ruling_dossier.py DIRECTORY`",
                     "python ruling_dossier.py pr"):
            self.assertRegex(text, pattern)
        for text in ("passes `ruling_dossier.py`.", "ruling_dossier.py reads structure only",
                     "`ruling_dossier.py`'s faults", "str(tools / \"ruling_dossier.py\")"):
            self.assertNotRegex(text, pattern)

    def test_no_current_caller_runs_a_moved_command(self):
        found = []
        for name, text in current_text_files():
            for module, command, replacement in MOVED:
                if name == module:
                    continue
                for number, line in enumerate(text.splitlines(), 1):
                    if invocation(module, command).search(line):
                        found.append(f"{name}:{number}: runs {Path(module).name} {command}; use {replacement}")
            for module, replacement in MOVED_PROGRAMS:
                if name == module:
                    continue
                for number, line in enumerate(text.splitlines(), 1):
                    if program_run(module).search(line):
                        found.append(f"{name}:{number}: runs {Path(module).name}; use {replacement}")
        self.assertEqual(found, [])

    def test_package_scripts_run_the_rust_commands(self):
        scripts = json.loads((REPO / "package.json").read_text(encoding="utf-8"))["scripts"]
        self.assertEqual(scripts["rates:check"], "python3 bench/tools/rust_runtime.py exec bench-tools -- rates check")
        self.assertEqual(scripts["audit:maintainers"],
                         "python3 bench/tools/rust_runtime.py exec bench-tools -- upstream report")
        self.assertEqual(scripts["evidence:fetch"], "python3 bench/tools/rust_runtime.py exec bench-tools -- evidence fetch "
                         "--manifest bench/evidence/manifests/repository-payloads-2026-10-08-v1.json")


if __name__ == "__main__":
    unittest.main()
