#!/usr/bin/env python3
"""Refuse a ruling dossier that does not show where a promise comes from and the searches made to find one.

    python3 bench/tools/ruling_dossier.py DIRECTORY

DIRECTORY holds one pull request's `summary.json` and `dossiers/<group>.md`, as docs/ruling-dossier-brief.md describes.
A recommendation to the user five times rested on a fact nobody had fetched
(docs/research/cohort-rebuild-2026-10-05/second-pass/DISCUSSION.md). This check makes each search a saved part of the
dossier: the query, the response, how many hits it gave and how many were read. It reads structure only; whether a
query was the right one is for the reader, who can now see it."""
import json
import sys
from pathlib import Path

WAYS = ("written", "announced", "built", "established", "practice")
CLASSES = ("public", "private", "deprecated", "removed", "other-purpose", "unstated")
FIELDS = ("made_by", "whose_interface", "classification", "source", "searched")
PLACES = ("project-docs", "owner-docs", "change", "maintainers", "public-code", "documented-way")
NOT_SORTED = ("refuted", "unproven", "outside-scope", "duplicate")
OUTCOMES = {("yes", "no"): "problem", ("yes", "yes"): "minor-defect", ("no", None): "suggestion"}


def search_faults(directory, group, place, search):
    if not isinstance(search, dict):
        return [f"{group}: promise.searched needs `{place}`: the query, the saved response, hits and read, or `none` with the reason"]
    if search.get("none"):
        return []
    found = []
    if not search.get("query"):
        found.append(f"{group}: searched.{place} needs the query as it was run")
    saved = search.get("saved")
    if not saved or not (directory / saved).is_file() or not (directory / saved).stat().st_size:
        found.append(f"{group}: searched.{place}.saved must name a saved, non-empty response under the dossier directory")
    hits, read = search.get("hits"), search.get("read")
    if not isinstance(hits, int) or not isinstance(read, int) or read > hits:
        found.append(f"{group}: searched.{place} needs `hits` and `read` as counts")
    elif read < hits and not search.get("unread"):
        found.append(f"{group}: searched.{place} read {read} of {hits} hits; read the rest or say in `unread` why not")
    if "found" not in search:
        found.append(f"{group}: searched.{place} needs `found`: the quotation, or null when the hits were read and silent")
    return found


def faults(directory):
    directory = Path(directory)
    found = []
    for entry in json.loads((directory / "summary.json").read_text(encoding="utf-8")):
        if entry.get("kind") != "candidate" or entry.get("recommendation") in NOT_SORTED:
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
        if promise["classification"] not in CLASSES:
            found.append(f"{group}: promise.classification must be one of {', '.join(CLASSES)}")
        if promised and not promise["source"]:
            found.append(f"{group}: a promise needs its source quoted")
        searched = promise["searched"] if isinstance(promise["searched"], dict) else {}
        for place in PLACES:
            found.extend(search_faults(directory, group, place, searched.get(place)))
        if promise["made_by"] == "practice" and not (searched.get("public-code") or {}).get("found"):
            found.append(f"{group}: a promise by practice needs the programs found in searched.public-code")
        delivered = entry.get("delivered")
        if delivered not in (("yes", "no") if promised else (None,)):
            found.append(f"{group}: `delivered` must be yes or no when promised and null when not")
        elif entry.get("recommendation") != OUTCOMES[("yes" if promised else "no", delivered)]:
            found.append(f"{group}: the recommendation must be `{OUTCOMES[('yes' if promised else 'no', delivered)]}`, "
                         f"which is what its promise and delivery say, or one of {', '.join(NOT_SORTED)}")
        dossier = directory / "dossiers" / f"{group}.md"
        text = dossier.read_text(encoding="utf-8") if dossier.exists() else ""
        for heading in ("## Promised?",) + (("## Delivered?",) if promised else ()):
            if heading not in text:
                found.append(f"{group}: dossiers/{group}.md has no `{heading}` section")
    return found


if __name__ == "__main__":
    problems = faults(sys.argv[1])
    print("\n".join(problems) if problems else "every candidate dossier shows its promise and its searches")
    raise SystemExit(1 if problems else 0)
