#!/usr/bin/env python3
"""Prints one table from the grouping runs: each case against each state. usage: table.py <results directory>"""
import re
import sys
from pathlib import Path

results = Path(sys.argv[1])
states = ["head", "fix-M1", "fix-o1", "fix-o2", "fix-o3", "fix-o4", "fix-o1+M1", "fix-o2+M1", "fix-o3+M1", "fix-o4+M1"]
table, order = {}, []
for state in states:
    text = (results / f"known-{state}.txt").read_text(encoding="utf-8")
    for match in re.finditer(r"^(\S+)(?: \(control\))?  (FAULT SHOWN|fault not shown)$", text, flags=re.M):
        case = match.group(1)
        if case not in order:
            order.append(case)
        table.setdefault(case, {})[state] = "FAULT" if match.group(2) == "FAULT SHOWN" else "ok"
print("| case | " + " | ".join(states) + " |")
print("| --- |" + " --- |" * len(states))
for case in order:
    print(f"| {case} | " + " | ".join(table[case].get(state, "?") for state in states) + " |")
