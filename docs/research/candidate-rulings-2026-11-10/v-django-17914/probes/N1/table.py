"""Summarize results/*.txt: for each case, the step that decides it, per tree.

Usage: python3 -I table.py <results dir>
A cell is the session's time zone (or role for role cases), "!" when it differs
from the step's expected value, or the error.
"""
import json
import sys
from pathlib import Path

results = Path(sys.argv[1])
DECIDING = {
    "v16": ["main-under-override"],
    "v16-idle": ["main-under-override"],
    "stale-a": ["B-under-override", "A-under-override"],
    "stale-b": ["B-under-override", "A-under-override"],
    "stale-copy": ["main-under-override", "copy-under-override"],
    "copy-role-1": ["copy", "ordinary wrapper"],
    "copy-role-2": ["ordinary wrapper", "copy"],
    "copy-tz-1": ["copy", "ordinary wrapper"],
    "copy-tz-2": ["ordinary wrapper", "copy"],
    "alias-tz": ["default", "other"],
    "alias-role": ["default", "other"],
    "v10": ["next-borrower"],
    "one-thread": ["main-under-override"],
}


def cell(row):
    r = row.get("result") or {}
    if "error" in r:
        return "ERROR " + r["error"].split(":")[0]
    value = r["current_user"] if "role" in row["case"] else r["timezone"]
    exp = row.get("expected")
    return value + (" !" if exp is not None and value != exp else "")


def load(path):
    rows, tree = {}, None
    for line in path.read_text().splitlines():
        if not line.startswith("{"):
            continue
        row = json.loads(line)
        if row["step"] == "identity":
            tree = row["tree"]
            continue
        rows[(tree, row["case"], row["step"])] = row
    return rows


rows = {}
for p in sorted(results.glob("result-*.txt")):
    rows.update(load(p))
trees = []
for (t, _, _) in rows:
    if t not in trees:
        trees.append(t)
print("case | step | " + " | ".join(trees))
for case, steps in DECIDING.items():
    for step in steps:
        cells = [cell(rows[(t, case, step)]) if (t, case, step) in rows else "-" for t in trees]
        print(f"{case} | {step} | " + " | ".join(cells))
