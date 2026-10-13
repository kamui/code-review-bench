#!/usr/bin/env python3
"""Summarise results/memory-<label>.txt files: one row per runtime, upload size, scenario, calls and hold.

    python3 memory-table.py results/memory-base.txt results/memory-head.txt [more ...]

Each cell lists the three fresh processes' figures in MiB above the process's own baseline: peak / retained."""
import json
import sys
from collections import defaultdict


def load(path):
    cells, label, failed = defaultdict(list), None, 0
    for line in open(path, encoding="utf-8"):
        if "exited with status" in line:
            failed += 1
        if not line.startswith("HTTP 200 {"):
            continue
        row = json.loads(line[len("HTTP 200 "):])
        label = row["label"]
        key = (row["runtime"], row["uploadMb"], row["scenario"], row["calls"], row["hold"])
        cells[key].append((row["peakOverBaselineMb"], row["retainedOverBaselineMb"], ",".join(row["bodyCacheKeys"])))
    return label, cells, failed


loaded = [load(path) for path in sys.argv[1:]]
keys = sorted({key for _, cells, _ in loaded for key in cells}, key=lambda k: (k[0], k[1], k[2], not k[4], k[3]))
print("| runtime | upload | scenario | calls | results kept | " + " | ".join(f"{label}: peak | {label}: retained" for label, _, _ in loaded) + " |")
print("| --- | --- | --- | --- | --- | " + " | ".join("--- | ---" for _ in loaded) + " |")
for key in keys:
    runtime, size, scenario, calls, hold = key
    cols = []
    for _, cells, _ in loaded:
        rows = cells.get(key, [])
        cols.append(", ".join(str(r[0]) for r in rows) or "-")
        cols.append(", ".join(str(r[1]) for r in rows) or "-")
    print(f"| {runtime} | {size} MB | {scenario} | {calls} | {'all' if hold else 'last only'} | " + " | ".join(cols) + " |")
print()
for label, cells, failed in loaded:
    kinds = sorted({row[2] for rows in cells.values() for row in rows})
    print(f"{label}: {sum(len(rows) for rows in cells.values())} processes reported, {failed} exited with an error; bodyCache keys seen: {kinds}")
