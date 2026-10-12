#!/usr/bin/env python3
"""Check that the filing which moved the re-cut tasks writes its files as one generation: all of them, or
none.

Usage: python3 bench/tools/test_last_push_move.py
Inputs: a copy of this repository's bench and docs trees in a temporary directory, made of hard links
where the file system allows, with its own copy of bench/grading and of the registry. The copy is first
put back to the selection the move published, without what the later review-code selection changed.
It is then put back to the records as they were before the move, from the retirement file and the
registry saved before publication, and the filing's own scripts run in it. Needs the freeze commits of
the selected runs in the clone. No network, and nothing is written into this checkout.
Exit codes: 0 all checks pass; 1 a test fails.
"""

from __future__ import annotations

import importlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import claims  # noqa: F401
import current_grading  # noqa: F401

ROOT = Path(__file__).resolve().parents[2]
FILING = Path("docs/research/last-push-rulings-2026-10-11/filing")
RECUT = Path("docs/research/last-push-recut-2026-10-07")
SELECTION = Path("docs/research/last-push-review-code-2026-10-12/selection")
GRADING, CURRENT = Path("bench/grading"), Path("bench/grading/current")
REGISTRY, INVENTORY = Path("bench/scoreboard.current.json"), CURRENT / "inventory.json"
RETIRED, RECEIPT = GRADING / "retired/last-push-recut.v1.json", GRADING / "rulings/cohort-rebuild.v8.md"
CARD = CURRENT / "impact-cards/GT-r9.json"
RECORDS = {"references": "targets", "adjudications": "decisions", "claims": "claims", "candidates": "candidates", "credits": "rulings"}
WRITTEN = [RETIRED, RECEIPT, CARD, REGISTRY, INVENTORY, *(CURRENT / f"{name}.json" for name in RECORDS)]


def link(source: str, target: str) -> None:
    try:
        os.link(source, target)
    except OSError:
        shutil.copy2(source, target)


def copy_of_the_repository(root: Path) -> None:
    """Link bench and docs into ``root`` and give it its own bytes of everything a filing writes."""
    def skip(directory: str, names: list) -> list:
        return [name for name in names if name == "__pycache__" or Path(directory, name) == ROOT / GRADING]
    for name in ("bench", "docs"):
        shutil.copytree(ROOT / name, root / name, copy_function=link, ignore=skip)
    shutil.copytree(ROOT / GRADING, root / GRADING)
    (root / REGISTRY).unlink()
    shutil.copy2(ROOT / REGISTRY, root / REGISTRY)
    git = subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "--absolute-git-dir"], text=True).strip()
    (root / ".git").write_text(f"gitdir: {git}\n", encoding="utf-8")


def filing_script(root: Path, name: str):
    """A filing script of the repository at ``root``, loaded so that it and the scripts it imports take that root."""
    paths = [str(root / FILING), *sys.path]
    with patch.object(sys, "path", paths), patch.dict(sys.modules):
        for loaded in ("record", "move", "stage_edition"):
            sys.modules.pop(loaded, None)
        return importlib.import_module(name)


def read(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def before_the_selection(root: Path, stage) -> None:
    """Put the copy back to the selection the move published. The review-code selection changed it afterwards: the
    links its intakes proposed leave the claims, and the registry and its inventory are the staged ones again."""
    plan = read(root / SELECTION / "plan.v1.json")
    proposed = {(link["claim"], link["review"]["path"], link["item_id"])
                for path in plan["intakes"] for link in read(root / path)["links"]}
    claims = read(root / CURRENT / "claims.json")
    for claim in claims["claims"]:
        claim["links"] = [link for link in claim["links"] if (claim["id"], link["review"]["path"], link["item_id"]) not in proposed]
    (root / CURRENT / "claims.json").write_text(json.dumps(claims, indent=2) + "\n", encoding="utf-8")
    (root / REGISTRY).write_text(stage.text(stage.staged_registry()[0], 2), encoding="utf-8")
    (root / INVENTORY).write_text(stage.text(current_grading.inventory(root), 2), encoding="utf-8")


def before_the_move(root: Path, move) -> None:
    """Put the copy's records back as they were before the move: the rows the filing adds are taken out, the
    retired rows and links return, every row of a moved task names its earlier revision, and the registry is the
    one saved before publication. The files the filing creates are removed."""
    retired, created = read(root / RETIRED), move.Move().created()
    former = {task["target"]: task["revision"] for task in retired["tasks"]}
    links = {row["claim"]: row["links"] for task in retired["tasks"] for row in task["links"]}
    documents = {name: read(root / CURRENT / f"{name}.json") for name in RECORDS}
    for name, field in RECORDS.items():
        rows = [row for row in documents[name][field] if row.get("id") not in created]
        gone = {"adjudications": "decisions"}.get(name, name)
        rows += [row["record"] for task in retired["tasks"] for row in task.get(gone, [])]
        for row in rows:
            if row["target"] in former:
                row["revision"] = dict(former[row["target"]])
                if name == "claims" and row["id"] in links:
                    row["links"] = links[row["id"]]
                if name == "references":
                    row["families"] = [family for family in row["families"] if family["id"] not in created]
        documents[name][field] = rows
        (root / CURRENT / f"{name}.json").write_text(json.dumps(documents[name], indent=2) + "\n", encoding="utf-8")
    (root / REGISTRY).write_bytes((root / RECUT / "registry-before-publication.v1.json").read_bytes())
    (root / INVENTORY).write_text("{}\n", encoding="utf-8")
    for path in (CARD, RECEIPT, RETIRED):
        (root / path).unlink()


def tree(root: Path) -> dict:
    """Every file a filing can write in the copy, with its bytes."""
    files = [path for path in (root / GRADING).rglob("*") if path.is_file()] + [root / REGISTRY]
    return {path.relative_to(root).as_posix(): path.read_bytes() for path in files}


def differing(one: dict, other: dict) -> list:
    """The files whose bytes differ between two trees, or that only one of them has."""
    return sorted(name for name in one.keys() | other.keys() if one.get(name) != other.get(name))


class Failing:
    """Stands in for ``record.replace``: lists the files written and fails the write numbered ``at``, once, so the
    writes that put files back afterwards go through."""

    def __init__(self, replace, at=None):
        self.replace, self.at, self.paths = replace, at, []

    def __call__(self, path, data):
        if len(self.paths) == self.at:
            self.at = None
            raise OSError("injected failure")
        self.paths.append(path)
        self.replace(path, data)


class OneGenerationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temp.cleanup)
        cls.root = Path(cls.temp.name).resolve()
        copy_of_the_repository(cls.root)
        cls.move = filing_script(cls.root, "move")
        before_the_selection(cls.root, cls.move.stage)
        cls.published = tree(cls.root)
        before_the_move(cls.root, cls.move)
        cls.before = tree(cls.root)

    def setUp(self) -> None:
        for name in tree(self.root).keys() - self.before.keys():
            (self.root / name).unlink()
        for name, data in self.before.items():
            path = self.root / name
            if not path.exists() or path.read_bytes() != data:
                path.write_bytes(data)

    def run_filing(self, at=None) -> list:
        """Run the copy's filing and return the files it wrote, in order. ``at`` fails that write."""
        writes = Failing(self.move.record.replace, at)
        with patch.object(self.move.record, "replace", writes):
            self.move.Move().run()
        return [path.relative_to(self.root).as_posix() for path in writes.paths]

    def test_the_filing_rebuilds_the_committed_files_from_the_records_before_the_move(self) -> None:
        written = self.run_filing()
        self.assertEqual(sorted(written), sorted(path.as_posix() for path in WRITTEN))
        after = tree(self.root)
        self.assertEqual(differing(after, self.before), sorted(written))
        self.assertEqual([path.as_posix() for path in WRITTEN if after[path.as_posix()] != self.published[path.as_posix()]], [])
        self.assertEqual(self.run_filing(), [])
        self.assertEqual(differing(tree(self.root), after), [])

    def test_a_failure_before_the_first_write_or_between_two_writes_leaves_every_file_as_it_was(self) -> None:
        written = self.run_filing()
        self.assertEqual(len(written), len(WRITTEN))
        for at, name in enumerate(written):
            self.setUp()
            with self.subTest(fails_writing=name, after_writing=written[:at]):
                with self.assertRaisesRegex(OSError, "injected failure"):
                    self.run_filing(at)
                self.assertEqual(differing(tree(self.root), self.before), [])

    def test_records_that_do_not_validate_leave_every_file_as_it_was(self) -> None:
        with patch.object(self.move.current, "validate_documents", side_effect=self.move.current.Inconsistent("injected")):
            with self.assertRaisesRegex(SystemExit, "do not validate"):
                self.run_filing()
        self.assertEqual(differing(tree(self.root), self.before), [])


class GenerationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.record = filing_script(ROOT, "record")
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        for name in ("changed", "removed", "same"):
            (self.root / name).write_text(f"{name} before", encoding="utf-8")
        self.before = {path.name: path.read_bytes() for path in self.root.iterdir()}

    def file(self, files) -> None:
        files.write(self.root / "changed", "changed after")
        files.write(self.root / "created", "created after")
        files.remove(self.root / "removed")
        files.write(self.root / "changed", "changed again")

    def test_an_error_restores_changed_and_removed_files_and_removes_created_ones(self) -> None:
        with self.assertRaisesRegex(ValueError, "stopped"):
            with self.record.Generation() as files:
                self.file(files)
                raise ValueError("stopped")
        self.assertEqual({path.name: path.read_bytes() for path in self.root.iterdir()}, self.before)

    def test_a_block_that_ends_without_an_error_keeps_what_it_wrote(self) -> None:
        with self.record.Generation() as files:
            self.file(files)
        self.assertEqual({path.name: path.read_text(encoding="utf-8") for path in self.root.iterdir()},
                         {"changed": "changed again", "created": "created after", "same": "same before"})

    def test_writing_the_bytes_a_file_already_has_writes_nothing(self) -> None:
        with patch.object(self.record, "replace", side_effect=AssertionError("a write began")):
            with self.record.Generation() as files:
                files.write(self.root / "same", "same before")
        self.assertEqual(files.before, {})

    def test_a_file_that_cannot_be_put_back_is_named_and_the_others_are_still_restored(self) -> None:
        replace = self.record.replace

        def refuse(path, data):
            if path.name == "changed" and data == b"changed before":
                raise OSError("no space")
            replace(path, data)
        with patch.object(self.record, "replace", refuse):
            with self.assertRaisesRegex(SystemExit, "could not be put back: .*changed: no space"):
                with self.record.Generation() as files:
                    self.file(files)
                    raise ValueError("stopped")
        self.assertEqual({name: (self.root / name).exists() for name in ("created", "removed")}, {"created": False, "removed": True})


if __name__ == "__main__":
    unittest.main(verbosity=2)
