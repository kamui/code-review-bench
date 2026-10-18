#!/usr/bin/env python3
"""Summarise time.ts output lines: one row per runtime, path, reader, mode, body and media-type case; one column per
variant. Each cell gives the median over processes of each process's median read time, then [lowest - highest] of
those process medians, in ms; 'cold' cells give the first request of each process instead.
Usage: python3 table.py [--cold] [--ratio] FILE... > table.md"""
import json
import statistics
import sys
from collections import defaultdict

cold = "--cold" in sys.argv
ratio = "--ratio" in sys.argv
files = [a for a in sys.argv[1:] if not a.startswith("--")]
cells, variants, rows, errors = defaultdict(list), [], [], defaultdict(int)
for name in files:
    for line in open(name):
        if not line.startswith("{"):
            continue
        r = json.loads(line)
        if r["error"] or r["readMs"] is None:
            errors[(r["label"], r["runtime"], r["body"], r["fileMb"], r["error"])] += 1
            continue
        body = r["body"] if r["body"] != "file" else f"multipart, {r['fileMb']} MB file"
        key = (r["runtime"], r["path"], r["op"], r["mode"], body, r["ct"])
        if key not in rows:
            rows.append(key)
        if r["label"] not in variants:
            variants.append(r["label"])
        cells[(key, r["label"])].append(r["coldMs"] if cold else r["readMs"]["median"])

def fmt(values):
    if not values:
        return ""
    med = statistics.median(values)
    digits = 3 if med < 1 else 1
    return f"{med:.{digits}f} [{min(values):.{digits}f}-{max(values):.{digits}f}] n={len(values)}"

head = ["runtime", "path", "reader", "body delivered as", "body", "media type"] + variants
if ratio:
    head.append("head / base (medians)")
print("| " + " | ".join(head) + " |")
print("| " + " | ".join("---" for _ in head) + " |")
for key in rows:
    row = list(key) + [fmt(cells.get((key, v), [])) for v in variants]
    if ratio:
        b, h = cells.get((key, "base")), cells.get((key, "head"))
        row.append(f"{statistics.median(h) / statistics.median(b):.2f}" if b and h else "")
    print("| " + " | ".join(row) + " |")
for (label, runtime, body, mb, error), count in errors.items():
    print(f"\nError in {count} process(es): {label} {runtime} {body} {mb} MB: {error}")
