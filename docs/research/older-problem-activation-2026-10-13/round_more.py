#!/usr/bin/env python3
"""Write ruling files 9 to 13 from the texts in round_more.texts.json and the before-records.

    python3 docs/research/older-problem-activation-2026-10-13/round_more.py

These are the questions prepared after the owner chose the admission test: the two tRPC admissions, their grouping,
and the Hono and soba admissions. The dossiers of these questions are written by hand. The first-answer tables, the
label tables and the route lines are read from the records and the record tool, as round.py reads them for
questions 1 to 5. Run it again after a record changes."""
import json
import subprocess
import sys
from pathlib import Path

from round import PARTY, ROUND, TEST, answers_table, bullets, label_part, route

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def grouping_table(record):
    rows = ["| Party | Answer | The same lines? | One fix? | One sentence? | Confidence | Reason |", "| --- | --- | --- | --- | --- | --- | --- |"]
    for answer in record["answers"]:
        tests = answer["tests"]
        short = f" ({', '.join(answer['short_of_high'])})" if answer["short_of_high"] else ""
        rows.append(f"| {PARTY[answer['by']]} | {answer['outcome']} | {tests['same_lines']} | {tests['one_fix']} | {tests['one_sentence']} | "
                    f"{answer['confidence']}{short} | {answer['reason']} |")
    return "\n".join(rows)


def sections(text):
    return [part for heading, body in text["sections"] for part in (f"## {heading}", "", body if isinstance(body, str) else bullets(body), "")]


def closing(text, routes):
    return ["## Route", "", *routes,
            "## The question as it would be asked", "", f"\"{text['question']}\"", "", "Options:", "",
            "\n".join(f"{index}. {option}" for index, option in enumerate(text["options"], 1)), "",
            "What each choice does:", "", bullets(text["effects"]), "",
            "## Answer", "", "Not asked. The asking session writes the after-record when the owner has answered.", ""]


if __name__ == "__main__":
    for text in json.loads((HERE / "round_more.texts.json").read_text(encoding="utf-8")):
        group, name = text["group"], text["file"]
        record_path = f"{ROUND}/rulings/{name}.{group}.before.json"
        record = json.loads((ROOT / record_path).read_text(encoding="utf-8"))
        verdict, reasons = route(record_path)
        head = [f"# Older problems, {text['heading']}", "", text["intro"], "", *sections(text)]
        if record["decision_type"] in ("older-grouping", "pre-existing-grouping"):
            body = ["## First answers", "", grouping_table(record), "", text["answers_note"], "",
                    "## Recommendation", "", text["recommendation"], "",
                    *closing(text, [f"`ruling_record.py route` prints **{verdict}**: " + "; ".join(reasons) + ".", "",
                                    "Grouping is always the owner's under rule Before 4. Nothing in this question could be settled by delegation.", ""])]
        else:
            body = ["## Who is hurt, and how badly", "", text["hurt"], "",
                    "## First answers on admission", "",
                    f"{TEST} The three answers were given under that test, with the rule text in force for \"Promised?\" and \"Delivered?\" below it "
                    "([`../assessors/rule-admission.v2.md`](../assessors/rule-admission.v2.md)).", "",
                    answers_table(record, group), "", text["answers_note"], "",
                    "## Recommendation on admission", "", text["recommendation"], "",
                    label_part(name, group, text["label"]),
                    *closing(text, [f"For the admission `ruling_record.py route` prints **{verdict}**: " + "; ".join(reasons) + ".", "",
                                    "An admission and a label stay with the owner whatever the parties answer. Nothing in this question could be settled by "
                                    "delegation.", ""])]
        (HERE / "rulings" / f"{name}.md").write_text("\n".join(head + body), encoding="utf-8")
        print(name, verdict)
    # The answers are the owner's, from the log, and filing/receipts.py puts them at the end of each file.
    subprocess.run([sys.executable, str(HERE / "filing/receipts.py"), "--answers"], check=True, cwd=ROOT)
