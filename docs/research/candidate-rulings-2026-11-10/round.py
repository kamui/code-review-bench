"""Write one ruling file per question into rulings/, from `bench-tools ruling check` and `ruling route` on each
before-record. A file that exists is not replaced. Run from the repository root:

    BENCH_RUNTIME_STORE=$HOME/.t3/bench-runtime python3 -I docs/research/candidate-rulings-2026-11-10/round.py"""
import json
import os
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

R = Path("docs/research/candidate-rulings-2026-11-10/rulings")
TOOL = "bench/tools/bench-tools"


def run(*args):
    done = subprocess.run([TOOL, "ruling", *args], capture_output=True, text=True)
    if done.returncode:
        sys.exit(f"{' '.join(args)} exited {done.returncode}: {done.stderr}")
    return done.stdout.rstrip("\n")


questions = defaultdict(list)
for path in sorted(R.glob("*.before.json")):
    record = json.loads(path.read_text(encoding="utf-8"))
    questions[record["ruling"]].append((record, path))
for ruling, records in questions.items():
    out = R / f"{ruling}.md"
    if out.exists():
        sys.exit(f"refusing to replace {out}")
    target = records[0][0]["target"]
    lines = [f"# Ruling {ruling}: {target}", "",
             "Decisions in this question: " + "; ".join(f"`{record['group']}` (candidate)" for record, _ in records) + ".", ""]
    for record, path in records:
        lines += [f"## `{record['group']}`: candidate", "", f"Record: [`{path.name}`]({path.name})", "",
                  "`bench-tools ruling check`:", "", "```text", run("check", str(path)), "```", "",
                  "`bench-tools ruling route`:", "", "```text", run("route", str(path)), "```", ""]
    with open(out, "x", encoding="utf-8") as f:
        f.write("\n".join(lines))
    print(out)
