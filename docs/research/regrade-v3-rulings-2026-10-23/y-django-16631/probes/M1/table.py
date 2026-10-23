"""Summarise probe outputs into results/table.md. Usage: python3 table.py <results dir>"""
import re
import sys
from pathlib import Path

results = Path(sys.argv[1])


def scenarios(text):
    out, current = {}, None
    for line in text.splitlines():
        m = re.match(r"^(S\d+L?)\s", line)
        if m:
            current = m.group(1)
            out[current] = []
        elif current and line.startswith("    "):
            out[current].append(line.strip())
    return out


def status(line):
    return re.search(r"HTTP (\d+)", line).group(1)


def user(line):
    m = re.search(r"user=(\S+?)[ ;]", line)
    return "signed in" if m and m.group(1) == "pat" else "signed out"


def cart_kept(line):
    return "item added by request 2" in line


def row(name):
    s = scenarios((results / f"{name}.txt").read_text())
    s7_file = results / f"{name.replace('-cache', '-db')}-save-every.txt"
    s7 = scenarios(s7_file.read_text())["S7"] if s7_file.exists() and name.endswith("-db") else None
    r = {}
    r["S1 req 2"] = status(s["S1"][1])
    r["S1 next, resp 1 last"] = user(s["S1"][2]) + (", write kept" if cart_kept(s["S1"][2]) else ", write lost")
    r["S1 next, resp 2 last"] = user(s["S1"][3]) + (", write kept" if cart_kept(s["S1"][3]) else ", write lost")
    r["S7 req 2 (save-every)"] = status(s7[1]) if s7 else "not run"
    r["S3 next, resp 2 last"] = user(s["S3"][3])
    r["S4 req 2"] = user(s["S4"][1])
    r["S5 next"] = user(s["S5"][1])
    r["S6 next"] = user(s["S6"][1])
    r["S8 req 2 (login() from anonymous, no rotation)"] = status(s["S8"][1])
    r["S0L req 2"] = status(s["S0L"][1])
    r["S0L next"] = user(s["S0L"][2])
    return r


names = []
for prefix in ["base", "head", "fix-gt-y2", "fix-m1-middleware", "both", "fix-gt-y2-keep-old-row"]:
    for backend in ["db", "cache", "cached_db", "file", "signed_cookies"]:
        if (results / f"{prefix}-{backend}.txt").exists():
            names.append(f"{prefix}-{backend}")

rows = {n: row(n) for n in names}
cols = list(next(iter(rows.values())).keys())
lines = ["| run | " + " | ".join(cols) + " |", "|" + " --- |" * (len(cols) + 1)]
for n, r in rows.items():
    lines.append(f"| {n} | " + " | ".join(r[c] for c in cols) + " |")
(results / "table.md").write_text("\n".join(lines) + "\n")
print("\n".join(lines))
