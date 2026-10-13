#!/usr/bin/env python3
"""Summarise results/time-<label>.txt files: one row per runtime, body, scenario and number of calls.

    python3 table.py results/time-base.txt results/time-head.txt [more ...]

Each cell is the median over the fresh processes of that cell of the per-process median, with the lowest and highest
per-process median in brackets. `request` is the whole request in milliseconds; `later calls` is the sum of every
parseBody() call after the first in the `repeat` scenario and the handler's parseBody() call in the `validator`
scenario."""
import json
import statistics
import sys
from collections import defaultdict


def load(path):
    cells, counts, label = defaultdict(list), {}, None
    for line in open(path, encoding="utf-8"):
        if not line.startswith("{"):
            continue
        row = json.loads(line)
        label = row["label"]
        key = (row["runtime"], row["body"], row["fileBytes"], row["scenario"], row["calls"])
        if row["platformParsesPerRequest"] is not None:
            counts[key] = row["platformParsesPerRequest"]
            continue
        later = row["callMs"] if row["scenario"] != "repeat" else row["callMs"][1:]
        cells[key].append((row["requestMs"]["median"], sum(call["median"] for call in later), row["callMs"][0]["median"]))
    return label, cells, counts


def cell(values):
    if not values:
        return "-"
    return f"{statistics.median(values):.3f} [{min(values):.3f}-{max(values):.3f}]"


loaded = [load(path) for path in sys.argv[1:]]
keys = sorted({key for _, cells, _ in loaded for key in cells}, key=lambda k: (k[0], k[2], k[1], k[3], k[4]))
print("| runtime | body | scenario | calls | " + " | ".join(f"{label}: request ms | {label}: later calls ms" for label, _, _ in loaded) + " |")
print("| --- | --- | --- | --- | " + " | ".join("--- | ---" for _ in loaded) + " |")
for key in keys:
    runtime, body, size, scenario, calls = key
    name = body if not size else f"multipart, {size // 1048576} MB file"
    cols = []
    for _, cells, _ in loaded:
        rows = cells.get(key, [])
        cols.append(cell([r[0] for r in rows]))
        cols.append(cell([r[1] for r in rows]) if (scenario != "repeat" or calls > 1) else "n/a")
    print(f"| {runtime} | {name} | {scenario} | {calls} | " + " | ".join(cols) + " |")
print()
print("Platform form parses per request (counting pass):")
count_keys = sorted({key for _, _, counts in loaded for key in counts}, key=lambda k: (k[0], k[2], k[1], k[3], k[4]))
print("| runtime | body | scenario | calls | " + " | ".join(label for label, _, _ in loaded) + " |")
print("| --- | --- | --- | --- | " + " | ".join("---" for _ in loaded) + " |")
for key in count_keys:
    runtime, body, size, scenario, calls = key
    name = body if not size else f"multipart, {size // 1048576} MB file"
    print(f"| {runtime} | {name} | {scenario} | {calls} | " + " | ".join(str(counts.get(key, "-")) for _, _, counts in loaded) + " |")
