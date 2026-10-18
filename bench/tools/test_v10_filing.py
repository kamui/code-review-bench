#!/usr/bin/env python3
"""Check that the filing of the v10 regrade rulings writes its files as one generation: all of them, or none.

Usage: python3 bench/tools/test_v10_filing.py
Inputs: a copy of this repository's bench and docs trees in a temporary directory, as test_last_push_move.py makes
it. The copy is put back to the records before the filing with the filing's own ``--undo``, which reads
``before.v1.json``, and the filing runs in it. No network, and nothing is written into this checkout.
Exit codes: 0 all checks pass; 1 a test fails.
"""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

from test_last_push_move import V10, Failing, copy_of_the_repository, differing, tree

ROOT = Path(__file__).resolve().parents[2]
CURRENT = Path("bench/grading/current")
BEFORE = V10 / "before.v1.json"
INDEX = Path("docs/research/cohort-rebuild-2026-10-05/second-pass/terms/rulings.index.json")
OUTSIDE = (BEFORE, INDEX, V10 / "entries-for-approval.md")
RECORDS = ("references", "adjudications", "claims", "candidates", "credits")


def read(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def filing(root: Path):
    spec = importlib.util.spec_from_file_location("v10_filing_record", root / V10 / "record.py")
    module = importlib.util.module_from_spec(spec)
    with patch.object(sys, "path", list(sys.path)):
        spec.loader.exec_module(module)
    return module


def everything(root: Path) -> dict:
    """The files of ``tree`` and the filing's files outside bench/grading."""
    files = tree(root)
    files.update({name.as_posix(): (root / name).read_bytes() for name in OUTSIDE if (root / name).exists()})
    return files


class OneGenerationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temp.cleanup)
        cls.root = Path(cls.temp.name).resolve()
        copy_of_the_repository(cls.root)
        for name in OUTSIDE:
            (cls.root / name).unlink()
            shutil.copy2(ROOT / name, cls.root / name)
        cls.filing = filing(cls.root)
        cls.filed = everything(cls.root)
        cls.filing.Filing().undo()
        cls.before = everything(cls.root)
        saved = read(ROOT / BEFORE)
        cls.written = sorted([(CURRENT / f"{name}.json").as_posix() for name in saved["records"]]
                             + [(CURRENT / "decision-queue.json").as_posix(), *saved["cards"], *saved["created"],
                                BEFORE.as_posix(), INDEX.as_posix()])

    def setUp(self) -> None:
        for name in everything(self.root).keys() - self.before.keys():
            (self.root / name).unlink()
        for name, data in self.before.items():
            path = self.root / name
            if not path.exists() or path.read_bytes() != data:
                path.write_bytes(data)

    def run_filing(self, at=None) -> list:
        """Run the copy's filing and return the files it wrote, in order. ``at`` fails that write."""
        writes = Failing(self.filing.shared.replace, at)
        with patch.object(self.filing.shared, "replace", writes):
            self.filing.Filing().run()
        return [path.relative_to(self.root).as_posix() for path in writes.paths]

    def test_the_undone_records_hold_nothing_the_filing_adds(self) -> None:
        saved = read(ROOT / BEFORE)
        self.assertEqual(sorted(saved["records"]), sorted(RECORDS))
        self.assertEqual(differing(self.before, self.filed), self.written)
        for name in RECORDS:
            text = (self.root / CURRENT / f"{name}.json").read_text(encoding="utf-8")
            self.assertNotIn("cohort-rebuild.v11.md", text)
            for identifier in saved["records"][name]["added"]:
                self.assertNotIn(f'"id": "{identifier}"', text)

    def test_the_filing_rebuilds_the_committed_files_from_the_records_before_it(self) -> None:
        written = self.run_filing()
        self.assertEqual(sorted(written), self.written)
        self.assertEqual(differing(everything(self.root), self.filed), [])

    def test_a_failure_before_the_first_write_or_between_two_writes_leaves_every_file_as_it_was(self) -> None:
        written = self.run_filing()
        for at, name in enumerate(written):
            self.setUp()
            with self.subTest(fails_writing=name, after_writing=written[:at]):
                with self.assertRaisesRegex(OSError, "injected failure"):
                    self.run_filing(at)
                self.assertEqual(differing(everything(self.root), self.before), [])

    def test_records_that_do_not_validate_leave_every_file_as_it_was(self) -> None:
        with patch.object(self.filing.current, "validate_documents", side_effect=self.filing.current.Inconsistent("injected")):
            with self.assertRaisesRegex(SystemExit, "do not validate"):
                self.run_filing()
        self.assertEqual(differing(everything(self.root), self.before), [])

    def test_a_second_run_is_refused_while_the_filing_is_in_place(self) -> None:
        self.run_filing()
        after = everything(self.root)
        with self.assertRaisesRegex(SystemExit, "is saved: the filing is in place"):
            self.filing.Filing().run()
        self.assertEqual(differing(everything(self.root), after), [])

    def test_every_candidate_is_closed_and_every_new_claim_has_one_decision(self) -> None:
        self.run_filing()
        candidates = read(self.root / CURRENT / "candidates.json")["candidates"]
        self.assertEqual([row["id"] for row in candidates if row.get("decision") is None], [])
        decisions = {row["id"] for row in read(self.root / CURRENT / "adjudications.json")["decisions"]}
        added = read(self.root / BEFORE)["records"]["claims"]["added"]
        for claim in read(self.root / CURRENT / "claims.json")["claims"]:
            if claim["id"] in added:
                with self.subTest(claim=claim["id"]):
                    self.assertIn(claim["adjudication"], decisions)


if __name__ == "__main__":
    unittest.main()
