#!/usr/bin/env python3
"""Resolve a frozen run's packet and verify its original and replacement pins.

Imported by run_cell.py and both skill runners. Inputs are a run directory, its
manifest, a target directory and the repository root. Invalid pins raise
ValueError; unreadable inputs raise OSError. Tests: test_packet_selection.py.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def repository_path(root: Path, name: str) -> Path:
    relative = Path(name)
    path = (root / relative).resolve()
    if relative.is_absolute() or ".." in relative.parts or not path.is_relative_to(root.resolve()):
        raise ValueError(f"packet path must stay inside the repository: {name}")
    return path


def frozen_manifest(run_dir: Path, manifest: dict, root: Path) -> dict | None:
    freeze = manifest.get("freeze_commit")
    if not freeze:
        return None
    try:
        relative = run_dir.resolve().relative_to(root.resolve()) / "manifest.json"
    except ValueError:
        return None
    frozen = subprocess.run(["git", "-C", str(root), "show", f"{freeze}:{relative.as_posix()}"],
                            capture_output=True, text=True, encoding="utf-8")
    return json.loads(frozen.stdout) if frozen.returncode == 0 else None


def select(run_dir: Path, manifest: dict, target_dir: Path, root: Path) -> Path:
    target_path = target_dir / "target.json"
    target = json.loads(target_path.read_bytes())
    entries = [row for row in manifest["cohort"] if row["target"] == target["id"]]
    if len(entries) != 1:
        raise ValueError(f"{target['id']}: target must appear exactly once in the cohort")
    entry = entries[0]
    original = target_dir / "packet.md"
    if digest(original) != target["packet_sha256"]:
        raise ValueError(f"{target['id']}: original packet.md differs from target.json")
    if target["diff_manifest_sha256"] != entry["diff_manifest_sha256"]:
        raise ValueError(f"{target['id']}: target.json and the cohort disagree on the diff identity")
    selected = original
    frozen = frozen_manifest(run_dir, manifest, root)
    if "packet_replacements" in manifest or (frozen is not None and "packet_replacements" in frozen):
        pin = manifest.get("packet_replacements")
        if not manifest.get("freeze_commit") or not manifest.get("frozen_at"):
            raise ValueError("packet replacements require a frozen run")
        if frozen is None:
            raise ValueError("cannot read the run manifest at freeze_commit")
        frozen_entries = [row for row in frozen["cohort"] if row["target"] == target["id"]]
        if frozen.get("packet_replacements") != pin or frozen_entries != entries:
            raise ValueError("packet replacement pin or cohort entry differs from freeze_commit")
        replacement_path = repository_path(root, pin["path"])
        if digest(replacement_path) != pin["sha256"]:
            raise ValueError("packet replacement manifest differs from the frozen hash")
        replacements = json.loads(replacement_path.read_bytes())
        if replacements["schema_version"] != 1:
            raise ValueError("unsupported packet replacement schema_version")
        ids = [row["target"] for row in replacements["targets"]]
        if len(ids) != len(set(ids)):
            raise ValueError("duplicate packet replacement target")
        replacement = next((row for row in replacements["targets"] if row["target"] == target["id"]), None)
        if replacement is not None:
            if digest(target_path) != replacement["target_sha256"]:
                raise ValueError(f"{target['id']}: frozen target.json changed")
            if (repository_path(root, replacement["original"]["path"]) != original.resolve()
                    or replacement["original"]["sha256"] != target["packet_sha256"]):
                raise ValueError(f"{target['id']}: original packet identity differs from replacement manifest")
            selected = repository_path(root, replacement["replacement"]["path"])
            if digest(selected) != replacement["replacement"]["sha256"]:
                raise ValueError(f"{target['id']}: replacement packet differs from replacement manifest")
    if digest(selected) != entry["packet_sha256"]:
        raise ValueError(f"{target['id']}: selected packet differs from the frozen cohort hash")
    return selected
