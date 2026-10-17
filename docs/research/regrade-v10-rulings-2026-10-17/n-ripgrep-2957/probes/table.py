#!/usr/bin/env python3
"""Summarise the documentation-fix runs: for each fix and each kind of .zshrc, whether every layout the fix's words
allow completes on the first Tab. Writes M1a/results/fixes.md and M1b/results/fixes.md."""
import json
import sys
from pathlib import Path

here = Path(sys.argv[1])
for group, recipe in (("M1a", "fpath"), ("M1b", "source")):
    data = json.loads((here / group / "results" / "fixes.json").read_text())
    rows = {}
    for sc in data["scenarios"]:
        fix, state, _ = sc["id"].split("|")
        rows.setdefault((fix, state), []).append(sc)
    lines = [f"# {group}: documentation fixes for the {recipe} way, at the head ({data['rg']}, zsh {data['zsh']})", "",
             "A cell is `works` when every layout the fix's words allow completes on the first Tab with nothing "
             "printed at startup.", "",
             "| Fix | .zshrc | Layouts | First Tab | Printed at startup | Cell |", "| --- | --- | --- | --- | --- | --- |"]
    for (fix, state), scs in rows.items():
        ok = all(sc["first_tab"] == "completes" and not sc["startup_output"] for sc in scs)
        for sc in scs:
            layout = sc["title"].split(": ", 1)[1]
            printed = "; ".join(sc["startup_output"]) or "nothing"
            lines.append(f"| {fix} | {'runs compinit' if state == 'has-ci' else 'no compinit'} | {layout} | "
                         f"{sc['first_tab']} | {printed} | {'works' if ok else 'FAILS'} |")
    (here / group / "results" / "fixes.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    print()
