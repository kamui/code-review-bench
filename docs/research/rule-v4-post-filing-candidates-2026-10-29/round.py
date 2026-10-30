#!/usr/bin/env python3
"""Write each question's ruling file from its before-records.

    python3 docs/research/rule-v4-candidates-2026-10-27/round.py

A ruling file holds, for each decision of the question, what `bench-tools ruling route` prints for its before-record:
the route, the whole neutral case every blind party read, and each party's first answer. The owner's answer is
appended when it is given. A file that already holds an answer is left as it is."""
import json
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
LABEL = {"pre-existing-admission": "admission as a pre-existing problem", "pre-existing-raise": "credit for a pre-existing problem",
         "candidate": "candidate", "recovery": "credit for a known problem", "band": "label"}

for question in json.loads((HERE / "questions.json").read_text(encoding="utf-8"))["questions"]:
    out = HERE / "rulings" / f"{question['ruling']}.md"
    if out.exists() and "## The owner's answer" in out.read_text(encoding="utf-8"):
        continue
    target = question["decisions"][0]["target"]
    parts = [f"# Ruling {question['ruling']}: {target}", "",
             "Decisions in this question: " + "; ".join(f"`{d['group']}` ({LABEL[d['decision_type']]})" for d in question["decisions"]) + ".", ""]
    for decision in question["decisions"]:
        record = HERE / "rulings" / f"{question['ruling']}.{decision['group']}{'-label' if decision['decision_type'] == 'band' else ''}.before.json"
        routed = subprocess.run(["bench/tools/bench-tools", "ruling", "route", str(record.relative_to(ROOT))], cwd=ROOT, capture_output=True, text=True, check=True).stdout
        parts += [f"## `{decision['group']}`: {LABEL[decision['decision_type']]}", "", f"Record: [`{record.name}`]({record.name})", "", "```text", routed.split("\n\n", 1)[0], "```", "", routed.split("\n\n", 1)[1].strip(), ""]
    out.write_text("\n".join(parts) + "\n", encoding="utf-8")
    print(out.relative_to(ROOT))
