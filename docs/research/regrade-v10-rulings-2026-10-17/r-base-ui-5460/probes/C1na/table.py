"""Print the renders per keystroke of each PROBE line, one column per result file.

    python3 -I table.py result-base.txt result-head.txt ...
"""
import json
import sys


def rows(path):
    found = {}
    for line in open(path, encoding="utf-8"):
        if not line.startswith("PROBE "):
            continue
        _, name, payload = line.rstrip("\n").split(" ", 2)
        data = json.loads(payload)
        for part, keystrokes in data.items():
            for k in keystrokes:
                found[(name, part, k["typed"])] = k
    return found


files = sys.argv[1:]
tables = [rows(f) for f in files]
print("case | part | typed | " + " | ".join(f.removeprefix("result-").removesuffix(".txt") for f in files))
for key in tables[0]:
    cells = []
    for t in tables:
        k = t.get(key)
        cells.append("-" if k is None else f"{k['controlRenders']} (v{k['validateCalls']}{' D' if k['dirty'] else ''}{' F' if k['filled'] else ''}{' I' if k['invalid'] else ''})")
    print(f"{key[0]} | {key[1]} | {key[2]!r} | " + " | ".join(cells))
