#!/usr/bin/env python3
"""Collect the VERDICT lines of every result-<variant>.txt beside this file into table.json and
print a compact table (cases down, variants across).

    python3 -I table.py
"""
import json
import pathlib
import re

here = pathlib.Path(__file__).resolve().parent
order = ["base", "head", "GT-s1", "GT-s2", "GT-s3", "GT-s4", "GT-s5a", "GT-s5b", "PE-s1", "PE-s2", "PE-s3",
         "PE-s2+PE-s3", "PE-s4", "PE-s5", "P1", "P1+GT-s5a", "P1+GT-s5b", "P1+PE-s2", "P1+PE-s3",
         "P1+PE-s2+PE-s3", "PE-s2+GT-s5b", "PE-s2+GT-s5a"]
line = re.compile(r"VERDICT (.+?): (FAILS|OK|NOT REACHED) - (.*)$")
table, cases, own = {}, [], {}
for variant in order:
    path = here / f"result-{variant}.txt"
    if not path.exists():
        continue
    text = path.read_text(encoding="utf-8")
    for raw in text.splitlines():
        m = line.search(raw)
        if not m:
            continue
        case, word, seen = m.groups()
        if case not in cases:
            cases.append(case)
        table.setdefault(case, {})[variant] = {"verdict": word, "seen": seen}
    tail = text.split("(the pull request's own tests", 1)[-1]
    own[variant] = " ".join(l.strip() for l in tail.splitlines()[1:] if l.strip())[:200]
variants = [v for v in order if (here / f"result-{v}.txt").exists()]
json.dump({"variants": variants, "cases": cases, "table": table, "pull_request_tests": own},
          open(here / "table.json", "w", encoding="utf-8"), indent=1)
short = {"FAILS": "F", "OK": ".", "NOT REACHED": "-"}
width = max(len(c) for c in cases)
print(" " * width + "  " + "  ".join(f"{i:>2}" for i in range(len(variants))))
for case in cases:
    print(case.ljust(width) + "  " + "  ".join(f"{short[table[case][v]['verdict']] if v in table[case] else '?':>2}" for v in variants))
for i, v in enumerate(variants):
    print(f"{i:>2} = {v}; PR tests: {own[v]}")
