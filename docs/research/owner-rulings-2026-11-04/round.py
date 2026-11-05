#!/usr/bin/env python3
"""Write each candidate question's ruling file from its before-records.

    BENCH_RUNTIME_STORE=<store> python3 -I docs/research/owner-rulings-2026-11-04/round.py

The rule-v4 post-filing round's `round.py` for this round's two candidate questions. A ruling file holds, for each
decision of the question, what `bench-tools ruling route` prints for its before-record: the route, the whole neutral
case every blind party read, and each party's first answer. The owner's words are in `answers.md`. Question 3, rule
version 6, is a decision on the text of a rule and has no ruling file of this kind (`rulings/P03-rule-v6-split-claims.md`)."""
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
QUESTIONS = (("01-hono-gt-p4-after-boundary", "p-hono-5067", ("NC-2d6d8bcc45c7",)),
             ("02-requests-gt-i7-conn-ca-certs", "i-requests-6667", ("NC-2a3425e24057", "NC-e9470b7251c2")))

for ruling, target, groups in QUESTIONS:
    out = HERE / "rulings" / f"{ruling}.md"
    parts = [f"# Ruling {ruling}: {target}", "", "Decisions in this question: " + "; ".join(f"`{group}` (candidate)" for group in groups) + ".", ""]
    for group in groups:
        record = HERE / "rulings" / f"{ruling}.{group}.before.json"
        routed = subprocess.run(["bench/tools/bench-tools", "ruling", "route", str(record.relative_to(ROOT))], cwd=ROOT,
                                capture_output=True, text=True, check=True).stdout
        head, body = routed.split("\n\n", 1)
        parts += [f"## `{group}`: candidate", "", f"Record: [`{record.name}`]({record.name})", "", "```text", head, "```", "", body.strip(), ""]
    out.write_text("\n".join(parts) + "\n", encoding="utf-8")
    print(out.relative_to(ROOT))
