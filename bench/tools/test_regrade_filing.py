#!/usr/bin/env python3
"""Check that the filing of the regrade rulings writes its files as one generation: all of them, or none.

Usage: python3 bench/tools/test_regrade_filing.py
Inputs: a copy of this repository's bench and docs trees in a temporary directory, as
test_last_push_move.py makes it. The copy is put back to the records before the filing with the filing's
own ``--undo``, which reads ``before.v1.json``, and the filing runs in it. Needs the freeze commits of the
selected runs in the clone. No network, and nothing is written into this checkout.
Exit codes: 0 all checks pass; 1 a test fails.
"""

from __future__ import annotations

import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

from test_last_push_move import Failing, copy_of_the_repository, differing, regrade_filing, tree

ROOT = Path(__file__).resolve().parents[2]
FILING = Path("docs/research/regrade-rulings-2026-10-13/filing")
CURRENT = Path("bench/grading/current")
BEFORE = FILING / "before.v1.json"
RECORDS = ("references", "adjudications", "claims", "candidates", "credits")


def read(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def equivalent_links(root: Path) -> dict:
    """Each comment with the claims it is linked to, and the relation of each link."""
    linked = {}
    for claim in read(root / CURRENT / "claims.json")["claims"]:
        for link in claim["links"]:
            linked.setdefault((link["review"]["path"], link["item_id"]), []).append((claim["id"], link["relation"]))
    return linked


class OneGenerationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temp.cleanup)
        cls.root = Path(cls.temp.name).resolve()
        copy_of_the_repository(cls.root)
        (cls.root / BEFORE).unlink()
        shutil.copy2(ROOT / BEFORE, cls.root / BEFORE)
        cls.filing = regrade_filing(cls.root)
        cls.filed = tree(cls.root)
        cls.filing.Filing().undo()
        cls.before = tree(cls.root)
        saved = read(ROOT / BEFORE)
        cls.written = sorted([(CURRENT / f"{name}.json").as_posix() for name in saved["records"]]
                             + [(CURRENT / "decision-queue.json").as_posix(), *saved["cards"], *saved["created"]])

    def setUp(self) -> None:
        for name in tree(self.root).keys() - self.before.keys():
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
            self.assertNotIn("cohort-rebuild.v9.md", text)
            for identifier in saved["records"][name]["added"]:
                self.assertNotIn(f'"id": "{identifier}"', text)

    def test_the_filing_rebuilds_the_committed_files_from_the_records_before_it(self) -> None:
        written = self.run_filing()
        self.assertEqual(sorted(written), self.written)
        after = tree(self.root)
        self.assertEqual(differing(after, self.before), self.written)
        self.assertEqual(differing(after, self.filed), [])
        self.assertEqual(self.run_filing(), [])
        self.assertEqual(differing(tree(self.root), after), [])

    def test_a_failure_before_the_first_write_or_between_two_writes_leaves_every_file_as_it_was(self) -> None:
        written = self.run_filing()
        for at, name in enumerate(written):
            self.setUp()
            with self.subTest(fails_writing=name, after_writing=written[:at]):
                with self.assertRaisesRegex(OSError, "injected failure"):
                    self.run_filing(at)
                self.assertEqual(differing(tree(self.root), self.before), [])

    def test_records_that_do_not_validate_leave_every_file_as_it_was(self) -> None:
        with patch.object(self.filing.current, "validate_documents", side_effect=self.filing.current.Inconsistent("injected")):
            with self.assertRaisesRegex(SystemExit, "do not validate"):
                self.run_filing()
        self.assertEqual(differing(tree(self.root), self.before), [])

    def test_a_changed_plan_is_refused_while_the_receipt_is_saved(self) -> None:
        self.run_filing()
        after = tree(self.root)
        filing = self.filing.Filing()
        filing.widened[0]["what_changed"] += " More."
        with self.assertRaisesRegex(SystemExit, "is saved and this filing would write it differently"):
            filing.run()
        self.assertEqual(differing(tree(self.root), after), [])

    def test_no_comment_the_filing_links_is_equivalent_to_a_claim(self) -> None:
        self.run_filing()
        plan = read(self.root / FILING / "plan.v1.json")
        filed = {entry["claim"] for part in ("new", "widened", "off_key") for entry in plan[part]}
        for comment, links in equivalent_links(self.root).items():
            if filed & {claim for claim, _relation in links}:
                with self.subTest(comment=comment):
                    self.assertEqual([claim for claim, relation in links if relation == "equivalent"], [])


if __name__ == "__main__":
    unittest.main()
