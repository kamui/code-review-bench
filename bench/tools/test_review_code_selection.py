#!/usr/bin/env python3
"""Check that the change which took the review-code reviews of the re-cut tasks into the published selection writes
the committed files, as one generation: all of them, or none.

Usage: python3 bench/tools/test_review_code_selection.py
Inputs: a copy of this repository's bench and docs trees in a temporary directory, as test_last_push_move.py makes
it. The copy is put back to the selection the re-cut record staged and to the claims without the links this change
added, and the change's own script runs in it. Needs the freeze commits of the selected runs in the clone. No
network, and nothing is written into this checkout.
Exit codes: 0 all checks pass; 1 a test fails.
"""

from __future__ import annotations

import hashlib
import importlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import current_grading  # noqa: F401
from test_last_push_move import (CURRENT, INVENTORY, RECUT, REGISTRY, ROOT, SELECTION, Failing, before_the_selection,
                                 copy_of_the_repository, differing, read, tree)

WRITTEN = [REGISTRY, INVENTORY]
LEFT = "bench/runs/2026-09-24-builtin-baseline/attempts/att-083/normalized.json"


def selection_script(root: Path):
    """The change's script in the repository at ``root``, loaded so that it and the scripts it imports take that root."""
    with patch.object(sys, "path", [str(root / SELECTION), *sys.path]), patch.dict(sys.modules):
        for loaded in ("record", "selection", "stage_edition"):
            sys.modules.pop(loaded, None)
        return importlib.import_module("selection")


class OneGenerationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temp.cleanup)
        cls.root = Path(cls.temp.name).resolve()
        copy_of_the_repository(cls.root)
        cls.selection = selection_script(cls.root)
        before_the_selection(cls.root, cls.selection.stage)
        cls.before = tree(cls.root)

    def setUp(self) -> None:
        for name in tree(self.root).keys() - self.before.keys():
            (self.root / name).unlink()
        for name, data in self.before.items():
            path = self.root / name
            if not path.exists() or path.read_bytes() != data:
                path.write_bytes(data)

    def run_change(self, at=None) -> list:
        """Run the copy's change and return the files it wrote, in order. ``at`` fails that write."""
        writes = Failing(self.selection.record.replace, at)
        with patch.object(self.selection.record, "replace", writes):
            self.selection.Selection().run()
        return [path.relative_to(self.root).as_posix() for path in writes.paths]

    def edit(self, name: str, change) -> None:
        path = self.root / CURRENT / f"{name}.json"
        document = read(path)
        change(document)
        path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")

    def test_the_copy_starts_from_the_selection_the_recut_record_staged(self) -> None:
        staged = read(ROOT / RECUT / "staged-edition.v2.json")["staged"]
        self.assertEqual({"registry_sha256": hashlib.sha256(self.before[REGISTRY.as_posix()]).hexdigest(),
                          "inventory_sha256": hashlib.sha256(self.before[INVENTORY.as_posix()]).hexdigest()},
                         {name: staged[name] for name in ("registry_sha256", "inventory_sha256")})

    def test_the_change_rebuilds_the_committed_files_from_the_staged_selection(self) -> None:
        written = self.run_change()
        self.assertEqual(sorted(written), sorted(path.as_posix() for path in WRITTEN))
        after = tree(self.root)
        self.assertEqual(differing(after, self.before), sorted(written))
        self.assertEqual([path.as_posix() for path in WRITTEN if after[path.as_posix()] != (ROOT / path).read_bytes()], [])
        self.assertEqual(self.run_change(), [])
        self.assertEqual(differing(tree(self.root), after), [])

    def test_a_failure_before_the_first_write_or_between_two_writes_leaves_every_file_as_it_was(self) -> None:
        written = self.run_change()
        self.assertEqual(len(written), len(WRITTEN))
        for at, name in enumerate(written):
            self.setUp()
            with self.subTest(fails_writing=name, after_writing=written[:at]):
                with self.assertRaisesRegex(OSError, "injected failure"):
                    self.run_change(at)
                self.assertEqual(differing(tree(self.root), self.before), [])

    def test_records_that_do_not_validate_leave_every_file_as_it_was(self) -> None:
        with patch.object(self.selection.current, "validate_documents", side_effect=self.selection.current.Inconsistent("injected")):
            with self.assertRaisesRegex(SystemExit, "do not validate"):
                self.run_change()
        self.assertEqual(differing(tree(self.root), self.before), [])

    def test_a_claim_whose_comments_all_left_the_selection_stops_the_change(self) -> None:
        def strand(claims: dict) -> None:
            claim = next(row for row in claims["claims"] if row["id"] == "CL-j-void-assertion")
            claim["links"] = [link for link in claim["links"] if link["review"]["path"] == LEFT]
            self.assertTrue(claim["links"])
        self.edit("claims", strand)
        edited = tree(self.root)
        with self.assertRaisesRegex(SystemExit, "only the owner retires one: CL-j-void-assertion$"):
            self.run_change()
        self.assertEqual(differing(tree(self.root), edited), [])

    def test_a_ruling_on_a_comment_that_left_the_selection_stops_the_change(self) -> None:
        def move(credits: dict) -> None:
            credits["rulings"][0]["review"]["path"] = LEFT
        self.edit("credits", move)
        edited, ruling = tree(self.root), read(self.root / CURRENT / "credits.json")["rulings"][0]["id"]
        with self.assertRaisesRegex(SystemExit, f"only the owner retires one: {ruling}$"):
            self.run_change()
        self.assertEqual(differing(tree(self.root), edited), [])


class RecordTests(unittest.TestCase):
    def test_the_committed_record_is_the_one_the_script_derives(self) -> None:
        selection = selection_script(ROOT)
        self.assertEqual(selection.RECORD.read_text(encoding="utf-8"), selection.Selection().recorded())


if __name__ == "__main__":
    unittest.main(verbosity=2)
