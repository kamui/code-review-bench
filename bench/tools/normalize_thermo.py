#!/usr/bin/env python3
"""Validate the thermo skill's verbatim finding index and map it to the shared schema."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import sys


class NormalizeError(Exception):
    pass


def read_json(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise NormalizeError(f"cannot read {path}: {error}") from error


def relative_report(root: Path, value: object) -> tuple[str, str]:
    if not isinstance(value, str) or not value:
        raise NormalizeError("each indexed entry needs a non-empty report_path")
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts:
        raise NormalizeError(f"report_path escapes artifact root: {value!r}")
    path = (root / relative).resolve()
    if not path.is_relative_to(root.resolve()) or not path.is_file():
        raise NormalizeError(f"indexed report is missing or outside artifact root: {value!r}")
    return relative.as_posix(), path.read_text(encoding="utf-8")


def anchor(entry: dict, clone: Path | None) -> tuple[str | None, int | None, int | None]:
    file = entry.get("file")
    if file is not None and not isinstance(file, str):
        raise NormalizeError("file anchor must be a string or null")
    if file and clone and Path(file).is_absolute():
        try:
            file = os.path.relpath(file, clone)
        except ValueError:
            pass
    first, last = entry.get("line_start"), entry.get("line_end")
    for name, value in (("line_start", first), ("line_end", last)):
        if value is not None and (isinstance(value, bool) or not isinstance(value, int) or value < 1):
            raise NormalizeError(f"{name} must be a positive integer or null")
    if first is not None and last is not None and last < first:
        raise NormalizeError("line_end precedes line_start")
    return file, first, last


def normalize(root: Path, clone: Path | None) -> dict:
    root = root.resolve(strict=True)
    summary = root / "summary.md"
    index_path = root / "finding-index.json"
    if not summary.is_file():
        raise NormalizeError("summary.md is missing")
    summary_text = summary.read_text(encoding="utf-8")
    index = read_json(index_path)
    if not isinstance(index, dict) or not isinstance(index.get("findings"), list):
        raise NormalizeError("finding-index.json must contain a findings array")
    questions = index.get("questions", [])
    if not isinstance(questions, list):
        raise NormalizeError("finding-index.json questions must be an array")
    explicit_empty = bool(re.search(r"\bno\s+(?:actionable\s+)?findings\b", summary_text, re.I))
    notes: list[str] = []
    items = []

    for kind, rows in (("finding", index["findings"]), ("question", questions)):
        for number, entry in enumerate(rows, 1):
            if not isinstance(entry, dict):
                raise NormalizeError(f"{kind} {number} must be an object")
            quote, title = entry.get("quote"), entry.get("title")
            if not isinstance(quote, str) or not quote.strip():
                raise NormalizeError(f"{kind} {number} needs a non-empty exact quote")
            report_path, text = relative_report(root, entry.get("report_path"))
            if quote not in text or (title is not None and (not isinstance(title, str) or title not in text)):
                notes.append(f"unresolved: {kind} {number} quote/title is not verbatim in {report_path}")
            file, first, last = anchor(entry, clone)
            items.append({"file": file, "line_start": first, "line_end": last, "claim": quote,
                          "consequence": None, "proposed_fix": None, "native_priority": None,
                          "native_action": None, "native_confidence": None, "kind": kind,
                          "native_fields": {**entry, "report_path": report_path}})

    if not index["findings"] and not explicit_empty:
        notes.append("unresolved: empty findings array without an explicit no-findings statement in summary.md")
    if explicit_empty and index["findings"]:
        notes.append("unresolved: summary says there are no findings but finding index is non-empty")
    status = "unresolved" if notes else ("parsed" if index["findings"] else "empty")
    return {"arm": "codex-skill", "parse_status": status, "native_verdict": None,
            "verdict_source": None, "items": items, "parse_notes": notes,
            "native_payload": index}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--artifact-root", required=True, type=Path)
    parser.add_argument("--clone", type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    try:
        result = normalize(args.artifact_root, args.clone.resolve() if args.clone else None)
        args.out.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"{result['parse_status']}: {len(result['items'])} indexed entries")
        return 0 if result["parse_status"] != "unresolved" else 1
    except (NormalizeError, OSError) as error:
        print(f"normalize_thermo.py: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
