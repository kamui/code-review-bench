"""Usage: table.py <results-dir> <name>... ; prints a probe-by-variant table and writes table.json."""
import json, re, sys, pathlib
d = pathlib.Path(sys.argv[1]); names = sys.argv[2:]
table = {}
for n in names:
    cur = None
    for line in (d / f"result-{n}.txt").read_text().splitlines():
        m = re.match(r"## (\S+): (.*)", line)
        if m:
            cur = m.group(1); table.setdefault(cur, {})[n] = {"status": "compiles" if m.group(2) == "compiles" else "error", "errors": []}
            continue
        m = re.match(r"line (\d+) (TS\d+): (.*)", line)
        if m and cur:
            table[cur][n]["errors"].append({"line": int(m.group(1)), "code": m.group(2), "message": m.group(3)[:300]})
(d / "table.json").write_text(json.dumps(table, indent=1) + "\n")
print("| Probe | " + " | ".join(names) + " |"); print("| --- |" + " --- |" * len(names))
for probe, row in table.items():
    cells = []
    for n in names:
        c = row.get(n)
        cells.append("compiles" if c["status"] == "compiles" else "**" + ", ".join(sorted({e["code"] for e in c["errors"]})) + "**")
    print(f"| `{probe}` | " + " | ".join(cells) + " |")
