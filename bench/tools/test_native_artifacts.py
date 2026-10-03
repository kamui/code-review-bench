import json
from pathlib import Path
import tempfile
import unittest
import zipfile

import native_artifacts as artifacts


class NativeArtifactsTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.base = Path(temp.name)
        self.root = self.base / "work/reports"
        self.root.mkdir(parents=True)
        for name, data in {"review.json": b'{"status":"complete"}', "notes.md": b"Evidence",
                           "run/scratch/first/code.py": b"original source\n",
                           "run/scratch/second/code.py": b"original source\n",
                           "run/scratch/second/changed.py": b"modified source\n",
                           "run/scratch/probe.sh": b"#!/bin/sh\nexit 0\n"}.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        (self.root / "run/scratch/probe.sh").chmod(0o755)
        self.index = self.root.parent / artifacts.INDEX
        self.index.write_text(json.dumps({"root": str(self.root), "files": [
            {"path": p.relative_to(self.root).as_posix(), "bytes": p.stat().st_size, "sha256": artifacts.digest(p)}
            for p in sorted(self.root.rglob("*")) if p.is_file()]}))
        self.out = self.base / "filed"

    def file(self):
        pin = artifacts.file_artifacts(self.root, self.index, self.out, "review.json")
        (self.out / "attempt.json").write_text(json.dumps({"native_artifact_storage": pin}))
        return pin

    def test_round_trip_keeps_probes_edits_modes_and_reports_with_one_blob_per_content(self):
        pin = self.file()
        storage, files = artifacts.verify(self.out)
        self.assertEqual(len(storage["archived_paths"]), 4)
        with zipfile.ZipFile(self.out / artifacts.ARCHIVE) as archive:
            self.assertEqual(len(archive.namelist()), 3)
        self.assertEqual((self.out / "reports/review.json").read_bytes(), (self.root / "review.json").read_bytes())
        self.assertFalse((self.out / "reports/run/scratch").exists())
        self.assertEqual((self.out / artifacts.INDEX).read_bytes(), self.index.read_bytes())
        restored = self.base / "restored"
        artifacts.restore(self.out, restored)
        for name in files:
            self.assertEqual((restored / name).read_bytes(), (self.root / name).read_bytes())
            self.assertEqual((restored / name).stat().st_mode & 0o777, (self.root / name).stat().st_mode & 0o777)
        self.assertEqual(artifacts.file_artifacts(self.root, self.index, self.out, "review.json"), pin)
        with self.assertRaises(artifacts.ArtifactError):
            artifacts.restore(self.out, restored)

    def test_archive_and_manifest_are_deterministic(self):
        self.file()
        second = self.base / "second"
        artifacts.file_artifacts(self.root, self.index, second, "review.json")
        for name in (artifacts.ARCHIVE, artifacts.STORAGE):
            self.assertEqual((self.out / name).read_bytes(), (second / name).read_bytes())

    def test_corrupt_archive_or_receipt_is_refused_before_restoration(self):
        self.file()
        archive = self.out / artifacts.ARCHIVE
        original = archive.read_bytes()
        archive.write_bytes(original + b"corrupted")
        restored = self.base / "restore"
        with self.assertRaises(artifacts.ArtifactError):
            artifacts.restore(self.out, restored)
        self.assertFalse(restored.exists())
        archive.write_bytes(original)
        storage = self.out / artifacts.STORAGE
        storage.write_text(storage.read_text() + "\n")
        with self.assertRaises(artifacts.ArtifactError):
            artifacts.file_artifacts(self.root, self.index, self.out, "review.json")

    def test_changed_or_unindexed_sources_are_not_silently_dropped(self):
        path = self.root / "run/scratch/second/changed.py"
        old = path.read_bytes()
        path.write_bytes(b"changed after indexing")
        with self.assertRaises(artifacts.ArtifactError):
            self.file()
        self.assertFalse(self.out.exists())
        path.write_bytes(old)
        (self.root / "run/scratch/new.txt").write_text("new evidence")
        with self.assertRaises(artifacts.ArtifactError):
            self.file()
        self.assertFalse(self.out.exists())

    def test_missing_pinned_manifest_cannot_revert_to_loose_filing(self):
        self.file()
        (self.out / artifacts.STORAGE).unlink()
        with self.assertRaises(OSError):
            artifacts.file_artifacts(self.root, self.index, self.out, "review.json")
        self.assertFalse((self.out / "reports/run/scratch").exists())

    def test_repacked_wrong_content_is_rejected_against_original_index(self):
        self.file()
        archive_path = self.out / artifacts.ARCHIVE
        with zipfile.ZipFile(archive_path) as archive:
            blobs = {name: archive.read(name) for name in archive.namelist()}
        blobs[next(iter(blobs))] = b"replacement content"
        with zipfile.ZipFile(archive_path, "w") as archive:
            for name, content in blobs.items():
                archive.writestr(name, content)
        storage_path = self.out / artifacts.STORAGE
        storage = json.loads(storage_path.read_text())
        storage["archive"] = artifacts.reference(archive_path)
        storage_path.write_text(json.dumps(storage))
        (self.out / "attempt.json").write_text(json.dumps({
            "native_artifact_storage": artifacts.reference(storage_path)}))
        with self.assertRaisesRegex(artifacts.ArtifactError, "content differs from index"):
            artifacts.restore(self.out, self.base / "restored")
        self.assertFalse((self.base / "restored").exists())

    def test_symlinks_and_traversal_are_refused(self):
        (self.root / "link").symlink_to(self.root / "notes.md")
        with self.assertRaises(artifacts.ArtifactError):
            self.file()
        for name in ("/outside", "../outside", "run/../../outside", "run//file"):
            with self.subTest(name=name), self.assertRaises(artifacts.ArtifactError):
                artifacts.relative_path(name)

    def test_existing_loose_filing_keeps_its_layout(self):
        self.out.mkdir()
        (self.out / "attempt.json").write_text("{}")
        self.assertIsNone(artifacts.file_artifacts(self.root, self.index, self.out, "review.json"))
        self.assertTrue((self.out / "reports/run/scratch/first/code.py").is_file())
        self.assertFalse((self.out / artifacts.STORAGE).exists())


if __name__ == "__main__":
    unittest.main()
