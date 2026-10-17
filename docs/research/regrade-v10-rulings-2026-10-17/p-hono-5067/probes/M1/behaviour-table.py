#!/usr/bin/env python3
"""Tabulate results/behaviour-<variant>.txt: one row per runtime, body and case; one column per variant.
Usage: python3 behaviour-table.py results base head fix-... > results/behaviour-table.md"""
import re
import sys
from pathlib import Path

directory, variants = Path(sys.argv[1]), sys.argv[2:]
rows, cells = [], {}
for variant in variants:
    runtime = section = None
    for line in (directory / f"behaviour-{variant}.txt").read_text().splitlines():
        if line.startswith("runtime: "):
            runtime = line.split(": ", 1)[1].split(" ")[0]
        elif line.startswith("## "):
            section = line[3:].split(":")[0]
        elif ": " in line and runtime and section and not line.startswith("#"):
            case, result = line.split(": ", 1)
            result = re.sub(r"^HTTP 200 ", "", result)
            parses = re.search(r"platform parses: (\d+)", result)
            if result.startswith("THROWS"):
                short = "throws"
            elif re.search(r"(formdata-undici|WebkitFormBoundary)", result):
                short = "garbage key"
            elif result.startswith("{}"):
                short = "empty {}"
            elif '"edited"' in result:
                short = "right (edit kept)"
            elif "widened" in case:
                short = "edit lost"
            else:
                short = "right"
            key = (runtime, section, case.split("  [")[0] + ("  [" + case.split("  [")[1] if "  [" in case else ""))
            if key not in rows:
                rows.append(key)
            cells[(key, variant)] = f"{short}, {parses.group(1)} parse(s)" if parses else short
print("| runtime | body | case | " + " | ".join(variants) + " |")
print("| --- | --- | --- | " + " | ".join("---" for _ in variants) + " |")
for key in rows:
    print(f"| {key[0]} | {key[1]} | {key[2]} | " + " | ".join(cells.get((key, v), "") for v in variants) + " |")
