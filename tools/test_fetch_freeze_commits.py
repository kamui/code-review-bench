#!/usr/bin/env python3
"""Exercise freeze hydration with full and shallow local Git repositories."""
import contextlib
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import fetch_freeze_commits as freeze


class FreezeCommitTests(unittest.TestCase):
    def test_hydration_preserves_checkout_depth(self):
        for shallow in (False, True):
            with self.subTest(shallow=shallow), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                source = root / "source"
                source.mkdir()

                def git(directory, *args):
                    return subprocess.check_output([
                        "git", "-C", str(directory), "-c", "user.name=kamui",
                        "-c", "user.email=2255+kamui@users.noreply.github.com", *args,
                    ], stderr=subprocess.PIPE, text=True).strip()

                git(source, "init", "--initial-branch=main")
                (source / "main-only").write_text("main history")
                git(source, "add", ".")
                git(source, "commit", "-m", "test(bench): establish main history")
                git(source, "checkout", "-b", "freeze")
                (source / "freeze-only").write_text("pinned runtime")
                git(source, "add", ".")
                git(source, "commit", "-m", "test(bench): retain an off-main freeze commit")
                commit = git(source, "rev-parse", "HEAD")
                git(source, "checkout", "main")
                clone = root / "clone"
                depth = ["--depth=1"] if shallow else []
                git(root, "clone", "--single-branch", "--branch=main", *depth, source.as_uri(), str(clone))
                manifest = clone / "bench/runs/example/manifest.json"
                manifest.parent.mkdir(parents=True)
                manifest.write_text(json.dumps({"freeze_commit": commit, "packet_replacements": {}}))
                self.assertEqual(git(clone, "rev-parse", "--is-shallow-repository"), str(shallow).lower())
                with patch.object(freeze, "ROOT", clone), contextlib.redirect_stdout(io.StringIO()) as output:
                    self.assertFalse(freeze.present(commit))
                    self.assertEqual(freeze.main(), 0)
                    self.assertTrue(freeze.present(commit))
                    self.assertEqual(freeze.main(), 0)
                self.assertIn("1 freeze commits, 1 fetched, 0 absent", output.getvalue())
                self.assertIn("1 freeze commits, 0 fetched, 0 absent", output.getvalue())
                self.assertEqual(git(clone, "rev-parse", "--is-shallow-repository"), str(shallow).lower())


if __name__ == "__main__":
    unittest.main()
