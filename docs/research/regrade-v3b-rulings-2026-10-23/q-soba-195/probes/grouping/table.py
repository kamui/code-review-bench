"""Usage: python3 -I table.py RESULTS_DIR > table.md

Reads the sanitized outputs that run-variants.sh wrote (one file per variant) and prints one
Markdown table per case: for each variant, the BitBucket lines of the start-up log (the lines
before "using git executable"), whether the
BitBucket backup ran, how the run ended, the exit code, and the case-specific facts."""
import re
import sys
from pathlib import Path

ORDER = ["head", "fix-p1", "fix-peq5", "both-peq5+p1", "fix-peq1-envtrue", "fix-peq1-lfslines",
         "fix-peq2", "fix-peq3", "fix-peq4", "fix-wide"]


def parse(path):
    cases = {}
    for block in path.read_text().split("=== scenario ")[1:]:
        name, _, rest = block.partition("\n")
        out = rest.split("--- soba output\n", 1)[1].split("--- exit code:", 1)[0]
        code = re.search(r"--- exit code: (\d+)", rest).group(1)
        lines = [re.sub(r"^soba: <date> <time> [\w.]+:\d+: ", "", l) for l in out.splitlines()]
        startup = lines[:next((i for i, l in enumerate(lines) if l.startswith("using git executable")), len(lines))]
        bb_log = [l.replace("BitBucket ", "") for l in startup if l.startswith("BitBucket ")]
        ran = "yes" if "backing up BitBucket repos" in lines else "no"
        end = next((l for l in lines if l in ("backups complete", "all backups failed",
                                               "backups completed with errors")), None)
        if end is None:
            end = lines[-1] if lines else "(no output)"
        notes = []
        if any(l == "GitHub backup LFS: true" for l in lines):
            notes.append("GitHub LFS line")
        lfs = rest.split("--- git-lfs calls\n", 1)[1].split("--- every path", 1)[0].strip()
        if lfs:
            notes.append(f"git-lfs called ({len(lfs.splitlines())})")
        for l in lines:
            if l.startswith("GitHub compare method") or "diff remote method" in l:
                notes.append(l)
        cases[name] = (", ".join(bb_log) or "none", ran, end, code, "; ".join(notes))
    return cases


results = {p.stem: parse(p) for p in Path(sys.argv[1]).glob("*.txt")}
variants = [v for v in ORDER if v in results]
case_names = list(results["head"].keys())
for case in case_names:
    print(f"### {case}\n")
    print("| Variant | BitBucket lines at start-up | BitBucket backup ran | End of run | Exit | Other |")
    print("| --- | --- | --- | --- | --- | --- |")
    for v in variants:
        bb, ran, end, code, notes = results[v][case]
        print(f"| {v} | {bb} | {ran} | {end} | {code} | {notes} |")
    print()
