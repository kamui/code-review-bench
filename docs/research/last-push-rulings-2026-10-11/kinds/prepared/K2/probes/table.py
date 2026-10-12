"""Usage: python3 table.py <results-dir> <name>...   Prints one row per probe and one column per result file."""
import re
import sys

directory, names = sys.argv[1], sys.argv[2:]
rows = {}
for name in names:
    for line in open(f"{directory}/result-{name}.txt"):
        m = re.match(r"## (\S+): (.*)", line)
        if m:
            rows.setdefault(m.group(1), {})[name] = "compiles" if m.group(2) == "compiles" else m.group(2)
width = max(len(p) for p in rows)
print("probe".ljust(width), " | ".join(names))
for probe, row in rows.items():
    print(probe.ljust(width), " | ".join(row.get(n, "?") for n in names))
