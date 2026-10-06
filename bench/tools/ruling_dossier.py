#!/usr/bin/env python3
"""Refuse a ruling dossier that does not show where a promise comes from and the searches made to find one.

    python3 bench/tools/ruling_dossier.py DIRECTORY

DIRECTORY holds one pull request's `summary.json` and `dossiers/<group>.md`, as docs/ruling-dossier-brief.md describes.
A `refresh.json` beside them, in the same form, replaces a group's promise, delivery and recommendation and leaves
the first summary as it was written. Recommendations to the user five times rested on a fact nobody had fetched or
had read under the wrong question (docs/research/cohort-rebuild-2026-10-05/second-pass/DISCUSSION.md). This check
makes each search a saved part of the dossier: the query, the response, how many hits it gave and how many were read.
It reads structure only. Whether a query was the right one, and whether a quotation was read rightly, is for the
reader, who can now open both."""
import json
import sys
from pathlib import Path

WAYS = ("written", "announced", "built", "established")
CLASSES = ("public", "private", "deprecated", "removed", "other-purpose", "unstated")
FIELDS = ("made_by", "whose_interface", "classification", "source", "searched")
PLACES = ("project-docs", "owner-docs", "change", "maintainers", "public-code", "documented-way")
UNSORTED = ("refuted", "unproven", "outside-scope")
SORTED = {("yes", "no"): ("problem", "duplicate"), ("yes", "yes"): ("minor-defect", "duplicate"), ("no", None): ("suggestion", "relied-on")}


def entries(directory):
    """Candidate entries as they are ready to ask: the first summary with any refreshed promise laid over it."""
    directory = Path(directory)
    refresh = directory / "refresh.json"
    refreshed = {entry["group"]: entry for entry in json.loads(refresh.read_text(encoding="utf-8"))} if refresh.exists() else {}
    for entry in json.loads((directory / "summary.json").read_text(encoding="utf-8")):
        if entry.get("kind") == "candidate":
            yield {**entry, **refreshed.get(entry["group"], {})}


def blocked(entry):
    searched = (entry.get("promise") or {}).get("searched")
    return [place for place in PLACES if isinstance(searched, dict) and (searched.get(place) or {}).get("state") == "blocked"]


def search_faults(directory, group, place, search):
    state = search.get("state") if isinstance(search, dict) else None
    if state in ("not-applicable", "blocked"):
        return [] if search.get("reason") else [f"{group}: searched.{place} is {state} and needs the reason"]
    if state != "checked":
        return [f"{group}: searched.{place} needs `state`: checked, not-applicable or blocked"]
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
    for entry in entries(directory):
        if entry.get("recommendation") in UNSORTED:
            continue
        group, promise = entry["group"], entry.get("promise")
        if not isinstance(promise, dict) or any(field not in promise for field in FIELDS):
            found.append(f"{group}: needs `promise` with {', '.join(FIELDS)}, in summary.json or refresh.json")
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
        if entry.get("recommendation") == "relied-on" and not (searched.get("public-code") or {}).get("found"):
            found.append(f"{group}: `relied-on` needs the programs found in searched.public-code")
        if not promised and blocked(entry):
            found.append(f"{group}: a blocked search cannot support `none`; finish it or recommend unproven")
        delivered = entry.get("delivered")
        if delivered not in (("yes", "no") if promised else (None,)):
            found.append(f"{group}: `delivered` must be yes or no when promised and null when not")
        else:
            follows = SORTED[("yes" if promised else "no", delivered)]
            if entry.get("recommendation") not in follows:
                found.append(f"{group}: its promise and delivery say the recommendation is one of {', '.join(follows)}; "
                             f"otherwise {', '.join(UNSORTED)}")
        written = [directory / "dossiers" / name for name in (f"{group}.md", f"{group}-supplement.md")]
        text = "".join(path.read_text(encoding="utf-8") for path in written if path.exists())
        for heading in ("## Promised?",) + (("## Delivered?",) if promised else ()):
            if heading not in text:
                found.append(f"{group}: neither dossiers/{group}.md nor its supplement has a `{heading}` section")
    return found


if __name__ == "__main__":
    problems = faults(sys.argv[1])
    print("\n".join(problems) if problems else "preparation complete; whether each search and reading is right still needs review")
    raise SystemExit(1 if problems else 0)
