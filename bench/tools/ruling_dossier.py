#!/usr/bin/env python3
"""Refuse a ruling dossier that does not say where a promise comes from, or what was searched to find none.

    python3 bench/tools/ruling_dossier.py DIRECTORY

DIRECTORY holds one pull request's `summary.json` and `dossiers/<group>.md`, as docs/ruling-dossier-brief.md describes.
A recommendation to the user twice rested on real breakage with nobody having asked whose interface the trigger was
(docs/research/cohort-rebuild-2026-10-05/second-pass/DISCUSSION.md). This check makes the answer a required part of
the dossier. It reads structure only; whether the search was adequate is for the reader."""
import json
import sys
from pathlib import Path

WAYS = ("written", "announced", "built", "established", "practice")
FIELDS = ("made_by", "whose_interface", "source", "owner_statement", "practice", "searched")


def faults(directory):
    directory = Path(directory)
    found = []
    for entry in json.loads((directory / "summary.json").read_text(encoding="utf-8")):
        if entry.get("kind") != "candidate":
            continue
        group, promise = entry["group"], entry.get("promise")
        if not isinstance(promise, dict) or any(field not in promise for field in FIELDS):
            found.append(f"{group}: summary.json needs `promise` with {', '.join(FIELDS)}")
            continue
        promised = promise["made_by"] in WAYS
        if not promised and promise["made_by"] != "none":
            found.append(f"{group}: promise.made_by must be one of {', '.join(WAYS)} or none")
        if not promise["whose_interface"]:
            found.append(f"{group}: promise.whose_interface is empty")
        if promised and not promise["source"]:
            found.append(f"{group}: a promise needs its source quoted")
        if not promise["searched"]:
            found.append(f"{group}: promise.searched must list what was read, also when a promise was found")
        if entry.get("delivered") not in (("yes", "no") if promised else (None,)):
            found.append(f"{group}: `delivered` must be yes or no when promised and null when not")
        dossier = directory / "dossiers" / f"{group}.md"
        text = dossier.read_text(encoding="utf-8") if dossier.exists() else ""
        for heading in ("## Promised?",) + (("## Delivered?",) if promised else ()):
            if heading not in text:
                found.append(f"{group}: dossiers/{group}.md has no `{heading}` section")
    return found


if __name__ == "__main__":
    problems = faults(sys.argv[1])
    print("\n".join(problems) if problems else "every candidate dossier states its promise")
    raise SystemExit(1 if problems else 0)
