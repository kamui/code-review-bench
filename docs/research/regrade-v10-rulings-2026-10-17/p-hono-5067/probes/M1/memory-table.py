#!/usr/bin/env python3
"""Summarise results/memory.txt: peak and held MiB above baseline, lowest to highest of the processes, per runtime,
path and variant. Usage: python3 memory-table.py results/memory.txt > results/memory-table.md"""
import json
import re
import sys
from collections import defaultdict

cells, variants, rows = defaultdict(list), [], []
for line in open(sys.argv[1]):
    m = re.match(r"(\S+) (\S+) round=\d+ (\S+): (?:HTTP \d+ )?(\{.*\})\s*$", line)
    if not m:
        continue
    runtime, path, variant, data = m.group(1), m.group(2), m.group(3), json.loads(m.group(4))
    if data["fileMbParsed"] != 100:
        data = {"peakOverBaselineMb": None, "heldOverBaselineMb": None, "readMs": None}
    if (runtime, path) not in rows:
        rows.append((runtime, path))
    if variant not in variants:
        variants.append(variant)
    cells[(runtime, path, variant)].append(data)

def span(values):
    values = [v for v in values if v is not None]
    if not values:
        return "wrong result"
    lo, hi = min(values), max(values)
    return f"{lo}" if lo == hi else f"{lo} to {hi}"

for measure, title in (("peakOverBaselineMb", "Peak, MiB above baseline"), ("heldOverBaselineMb", "Held after the read, MiB above baseline"), ("readMs", "Time of the read, ms (includes making the upload's chunks)")):
    print(f"\n{title}, one 100 MB streamed upload, three fresh processes:\n")
    print("| runtime | path | " + " | ".join(variants) + " |")
    print("| --- | --- | " + " | ".join("---" for _ in variants) + " |")
    for runtime, path in rows:
        print(f"| {runtime} | {path} | " + " | ".join(span([d[measure] for d in cells[(runtime, path, v)]]) for v in variants) + " |")
