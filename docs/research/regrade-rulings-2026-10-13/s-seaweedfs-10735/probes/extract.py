#!/usr/bin/env python3
"""Cut one group's cases out of the shared M2 run and write the grouping rows that bear on it.

    python3 -I extract.py <probes directory>

M2a and M2b are two windows of one branch and were run by one probe (M2/zz_probe_m2_test.go).
This writes M2a/result-*.txt and M2b/result-*.txt as the header of the shared result plus the
subtests of that group, unchanged, and <group>/grouping.txt from grouping/table.json."""
import json
import re
import sys
from pathlib import Path

probes = Path(sys.argv[1])
CASES = {"M2a": ("A_", "C_", "D_", "E_", "G_"), "M2b": ("B_", "C_", "F_")}
ROWS = {
    "M1": ("M1 store page", "M1 filer hasMore", "M1 IsDirectoryEmpty"),
    "M2a": ("M2a",),
    "M2b": ("M2b",),
}
KNOWN = ("GT-s2", "GT-s3 site 1", "GT-s3 site 2", "GT-s4 recursive delete", "GT-s4 listing in the gap", "GT-s5")

for group, prefixes in CASES.items():
    for source in sorted((probes / "M2").glob("result-*.txt")):
        lines = source.read_text(encoding="utf-8").splitlines()
        out, keep = [], True
        for line in lines:
            start = re.match(r"=== RUN\s+TestProbeM2/(\S+)", line)
            if start:
                keep = start.group(1).startswith(prefixes)
            elif line.startswith(("--- ", "    --- ", "PASS", "ok ", "FAIL")):
                keep = line.startswith(("--- PASS: TestProbeM2 ", "PASS", "ok ", "FAIL")) or any(f"TestProbeM2/{p}" in line for p in prefixes)
            if keep:
                out.append(line)
        header = f"# extract of ../M2/{source.name}: the header and the subtests {', '.join(prefixes)} only, lines unchanged"
        (probes / group / source.name).write_text("\n".join([header] + out) + "\n", encoding="utf-8")

table = json.loads((probes / "grouping" / "table.json").read_text(encoding="utf-8"))
for group, rows in ROWS.items():
    text = [f"# rows of ../grouping/table.json that bear on {group}; patches and full outputs are in ../grouping/",
            "# a variant is the head with the named fix or fixes applied; 'base' is the commit before the change, unpatched", ""]
    for case in rows + KNOWN:
        text.append(case)
        for variant in table["variants"]:
            cell = table["table"][case].get(variant)
            if cell:
                text.append(f"  {variant:16} {cell['verdict']:11} {cell['seen']}")
        text.append("")
    (probes / group / "grouping.txt").write_text("\n".join(text), encoding="utf-8")
