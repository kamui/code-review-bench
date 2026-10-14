#!/usr/bin/env python3
"""Print the PROBE lines of result files side by side: M-table.py label=file [label=file ...]"""
import json, re, sys

def load(path):
    out = {}
    for line in open(path, encoding="utf-8"):
        m = re.match(r"PROBE (\S+) (.*)", line.strip())
        if m:
            out[m.group(1)] = json.loads(m.group(2))
    return out

sides = [(a.split("=", 1)[0], load(a.split("=", 1)[1])) for a in sys.argv[1:]]
for case in sides[0][1]:
    print("==", case)
    for step in sides[0][1][case]:
        for label, data in sides:
            value = data.get(case, {}).get(step)
            if isinstance(value, dict):
                value = " ".join(f"{k}={json.dumps(v)}" for k, v in value.items())
            else:
                value = json.dumps(value)
            print(f"  {step:34} {label:10} {value}")
