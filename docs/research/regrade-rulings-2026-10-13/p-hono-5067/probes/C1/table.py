#!/usr/bin/env python3
"""Summarise results/memory-<label>.txt files of probe.ts: one row per runtime, upload size and mode.

    python3 table.py results/memory-base.txt results/memory-head.txt [more ...]

Each cell lists the three fresh processes' figures in MiB above the process's own baseline: peak / retained.
The tracking pass is listed after the table."""
import json
import sys
from collections import defaultdict


def load(path):
    cells, tracked, label, failed = defaultdict(list), [], None, 0
    for line in open(path, encoding="utf-8"):
        if "exited with status" in line:
            failed += 1
        if not line.startswith("{"):
            continue
        row = json.loads(line)
        label = row["label"]
        if row["tracked"] is not None:
            tracked.append(row)
            continue
        cells[(row["runtime"], row["uploadMb"], row["mode"])].append((row["peakOverBaselineMb"], row["retainedOverBaselineMb"]))
    return label, cells, tracked, failed


loaded = [load(path) for path in sys.argv[1:]]
keys = sorted({key for _, cells, _, _ in loaded for key in cells}, key=lambda k: (k[0], k[1], k[2] != "plain", k[2]))
print("| runtime | upload | mode | " + " | ".join(f"{label}: peak | {label}: retained" for label, _, _, _ in loaded) + " |")
print("| --- | --- | --- | " + " | ".join("--- | ---" for _ in loaded) + " |")
for key in keys:
    runtime, size, mode = key
    cols = []
    for _, cells, _, _ in loaded:
        rows = cells.get(key, [])
        cols.append(", ".join(str(r[0]) for r in rows) or "-")
        cols.append(", ".join(str(r[1]) for r in rows) or "-")
    print(f"| {runtime} | {size} MB | {mode} | " + " | ".join(cols) + " |")
print()
print("Tracking pass, 100 MB upload: calls of Request.prototype.arrayBuffer, and whether the buffer it returned is still reachable after the parse and a forced collection, while the request and the parsed result are alive:")
for label, _, tracked, failed in loaded:
    for row in tracked:
        print(f"- {label}, {row['runtime']}, {row['mode']}: calls {row['tracked']['requestArrayBufferCalls']}, still reachable {row['tracked']['bufferStillReachable']}, bodyCache keys {row['bodyCacheKeys']}")
    print(f"- {label}: {failed} processes exited with an error")
