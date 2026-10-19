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
)
# Frozen runs, imported originals and research records keep the commands they recorded.
HISTORICAL = ("artifacts/", "bench/runs/", "docs/research/")


def invocation(module, command):
    """A shell command, a quoted argument list or prose that runs `module command`."""
    return re.compile(rf"\b{re.escape(Path(module).name)}[\"')\],\s`]+{re.escape(command)}\b")


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

    def test_no_current_caller_runs_a_moved_command(self):
        found = []
        for name, text in current_text_files():
            for module, command, replacement in MOVED:
                if name == module:
                    continue
                for number, line in enumerate(text.splitlines(), 1):
                    if invocation(module, command).search(line):
                        found.append(f"{name}:{number}: runs {Path(module).name} {command}; use {replacement}")
        self.assertEqual(found, [])

    def test_package_scripts_run_the_rust_commands(self):
        scripts = json.loads((REPO / "package.json").read_text(encoding="utf-8"))["scripts"]
        self.assertEqual(scripts["rates:check"], "python3 bench/tools/rust_runtime.py exec bench-tools -- rates check")
        self.assertEqual(scripts["audit:maintainers"],
                         "python3 bench/tools/rust_runtime.py exec bench-tools -- upstream report")


if __name__ == "__main__":
    unittest.main()
