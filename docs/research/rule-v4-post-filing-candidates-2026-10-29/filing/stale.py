#!/usr/bin/env python3
"""List the batches the filing made stale, with the measured list-price cost of each batch's last grade.

    bench/tools/bench-tools methodology --out <queue.json>
    python3 -I docs/research/rule-v4-post-filing-candidates-2026-10-29/filing/stale.py <queue.json> > docs/research/rule-v4-post-filing-candidates-2026-10-29/filing/stale.v1.md

Run from the repository root before `bench-tools grade invalidate`, which removes the grades these costs come from. A
batch is listed when the queue marks it `stale`. The Sol cost is its grade in `grades.json` (Codex gpt-6.1-sol, high,
rule version 4); the Opus cost is the latest Claude Opus 5.5 (high) receipt for the same batch under
`bench/grading/current/assessments/`."""
import json
import sys
from pathlib import Path

ROOT = Path.cwd()


def load(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def priced(receipt):
    return (load(ROOT / receipt)["provenance"].get("usage") or {}).get("priced_total_usd")


def latest_opus():
    found = {}
    for path in (ROOT / "bench/grading/current/assessments").glob("*/*/assessment-*/receipt.json"):
        provenance = load(path)["provenance"]
        if provenance.get("model") != "claude-opus-5-5":
            continue
        run, target, number = path.parts[-4], path.parts[-3], int(path.parts[-2].split("-")[1])
        key = (f"runs/{run}", target)
        if key not in found or number > found[key][0]:
            found[key] = (number, (provenance.get("usage") or {}).get("priced_total_usd"))
    return {key: cost for key, (_number, cost) in found.items()}


def money(value):
    return "unknown" if value is None else f"${value:.2f}"


def main():
    queue = load(sys.argv[1])
    stale = [(batch["run"], batch["target"]) for batch in queue["batches"] if batch["state"] == "stale"]
    others = sorted({batch["state"] for batch in queue["batches"]} - {"current", "stale"})
    if others:
        raise SystemExit(f"the queue holds batches that are {', '.join(others)}; this list covers stale batches only")
    grades = {(batch["run"], batch["target"]): batch["assessor"]["receipt"]["path"]
              for batch in load(ROOT / "bench/grading/current/grades.json")["batches"]}
    opus = latest_opus()
    rows = [(target, run, priced(grades[(run, target)]), opus.get((run, target))) for run, target in sorted(stale, key=lambda k: (k[1], k[0]))]
    lines = ["# Batches the post-filing candidate filing made stale", "",
             f"Written by [`stale.py`](stale.py) from `bench-tools methodology` after the filing and before `bench-tools grade invalidate`. "
             f"{len(rows)} of {len(queue['batches'])} selected batches are stale. Costs are measured list prices of each batch's last grade: "
             "Codex gpt-6.1-sol (high) under rule version 4, and the latest Claude Opus 5.5 (high) grade of the same batch, under rule "
             "version 3. They leave out failed sessions.", "",
             "| Pull request | Stale batches | Sol | Opus 5.5 high |", "| --- | ---: | ---: | ---: |"]
    for target in sorted({row[0] for row in rows}):
        mine = [row for row in rows if row[0] == target]
        total = sum(1 for batch in queue["batches"] if batch["target"] == target)
        sol = [row[2] for row in mine]
        op = [row[3] for row in mine]
        lines.append(f"| {target} | {len(mine)} of {total} | {money(sum(x for x in sol if x is not None))}"
                     f"{'' if None not in sol else ' (some unknown)'} | {money(sum(x for x in op if x is not None))}"
                     f"{'' if None not in op else ' (some unknown)'} |")
    lines.append(f"| All | {len(rows)} | {money(sum(row[2] or 0 for row in rows))} | {money(sum(row[3] or 0 for row in rows))} |")
    lines += ["", "| Pull request | Batch | Sol | Opus 5.5 high |", "| --- | --- | ---: | ---: |"]
    lines += [f"| {target} | `{run.removeprefix('runs/')}` | {money(sol)} | {money(op)} |" for target, run, sol, op in rows]
    print("\n".join(lines))


main()
