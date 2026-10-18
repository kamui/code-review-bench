#!/usr/bin/env python3
"""Summarise results/net.txt. For each runtime, upload size and variant: the median, lowest and highest of the time
spent in c.req.parseBody() and of curl's total time, over every upload after the first of each server process (the
first is shown apart as 'first upload'). Usage: python3 net-table.py results/net.txt > results/net-table.md"""
import json
import re
import statistics
import sys
from collections import defaultdict

warm, first, order = defaultdict(lambda: ([], [])), defaultdict(lambda: ([], [])), []
for line in open(sys.argv[1]):
    m = re.match(r"runtime=(\S+) variant=(\S+) uploadMb=(\d+) round=\d+ request=(\d+) (\{.*\}) totalSeconds=([\d.]+)", line)
    if not m:
        continue
    key = (m.group(1), int(m.group(3)), m.group(2))
    if key not in order:
        order.append(key)
    data = json.loads(m.group(5))
    target = first if m.group(4) == "1" else warm
    target[key][0].append(data["parseMs"])
    target[key][1].append(float(m.group(6)) * 1000)

def fmt(values):
    return f"{statistics.median(values):.0f} [{min(values):.0f}-{max(values):.0f}]" if values else ""

print("| runtime | upload | variant | parseBody() ms, later uploads | curl total ms, later uploads | parseBody() share of total | parseBody() ms, first upload | curl total ms, first upload |")
print("| --- | --- | --- | --- | --- | --- | --- | --- |")
for key in order:
    p, t = warm[key]
    share = f"{statistics.median(p) / statistics.median(t) * 100:.0f}%"
    print(f"| {key[0]} | {key[1]} MB | {key[2]} | {fmt(p)} | {fmt(t)} | {share} | {fmt(first[key][0])} | {fmt(first[key][1])} |")
