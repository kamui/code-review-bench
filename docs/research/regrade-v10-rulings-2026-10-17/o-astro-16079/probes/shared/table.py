#!/usr/bin/env python3
"""Prints a Markdown table: one row per case, one column per state, from results/all-<state>.txt.
usage: python3 -I table.py <results directory> <state,state,...> <tag> [<tag> ...]"""
import json
import re
import sys

results, states, tags = sys.argv[1], sys.argv[2].split(","), set(sys.argv[3:])
rows, order = {}, []
for state in states:
    case = None
    for line in open(f"{results}/all-{state}.txt", encoding="utf-8"):
        m = re.match(r"^(\S+) \[([^\]]*)\]\. ", line)
        if m:
            case = m.group(1) if tags & set(m.group(2).split()) else None
            if case and case not in order:
                order.append(case)
        elif case and line.strip().startswith("result:"):
            r = json.loads(line.split("result:", 1)[1])
            if "threw" in r:
                cell = "threw " + r["threw"]
            else:
                what = r.get("id") and f"id {r['id']}" or r.get("route") or r.get("body", "")
                cell = f"{r['status']} {what}"
                if r.get("location"):
                    cell += f" to {r['location']}"
                if r.get("cookieUser"):
                    cell += f" (for {r['cookieUser']})"
            rows.setdefault(case, {})[state] = cell
print("| case | " + " | ".join(states) + " |")
print("| --- |" + " --- |" * len(states))
for case in order:
    print(f"| {case} | " + " | ".join(rows[case].get(s, "") for s in states) + " |")
