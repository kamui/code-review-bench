#!/usr/bin/env python3
"""Write `round.md`: the hand-written `round.head.md` followed by tables generated from the round's data files.

    python3 docs/research/regrade-rulings-2026-10-13/round.py

It reads `inventory.v1.json`, `open-claims.v1.json`, `questions.json`, `routes.v1.json` and each pull request's
`summary.json`. The generated part holds the questions in the order to ask them, every comment behind each
candidate question, the credit pairs, and the open claims by repeated question. Edit `round.head.md`, never
`round.md`. Run from the repository root."""
import json
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def load(name):
    return json.loads((HERE / name).read_text(encoding="utf-8"))


def cell(text):
    return " ".join(str(text).split()).replace("|", "\\|")


def short(review):
    parts = Path(review).parts
    return f"{parts[2]} {parts[4]}"


def main():
    inventory, claims, questions, routes = load("inventory.v1.json"), load("open-claims.v1.json"), load("questions.json"), load("routes.v1.json")
    by_group = {(row["target"], row["group"]): row for row in inventory["questions"]}
    route = defaultdict(list)
    for row in routes:
        route[row["ruling"]].append(row)
    out = [(HERE / "round.head.md").read_text(encoding="utf-8").rstrip(), "", "# Generated tables", "", "Written by `round.py` from `inventory.v1.json`, `open-claims.v1.json`, `questions.json` and `routes.v1.json`. "
           "A review is named by its run directory and attempt.", "", "## The questions in the order to ask them", "",
           "| Ruling | Pull request | Decisions | First answers: asking session, Sol, Astra | Route |", "| --- | --- | --- | --- | --- |"]
    for question in questions:
        rows = route[f"regrade-{question['number']}"]
        answers = "; ".join(f"`{row['group']}`: " + ", ".join(f"{row['answers'][party]['pick']} ({row['answers'][party]['confidence']})" for party in ("recommender", "assessor-1", "assessor-2") if party in row["answers"]) for row in rows)
        out.append(f"| [{question['number']}](rulings/{question['number']:02d}-{question['slug']}.md) | `{question['target']}` | {cell(question['title'])} | {cell(answers)} | {', '.join(sorted({row['route'] for row in rows}))} |")
    out += ["", "## Every comment behind each candidate question", ""]
    for question in questions:
        for decision in question["decisions"]:
            entry = by_group.get((question["target"], decision.get("inventory_group", decision["group"])))
            if decision["decision_type"] != "candidate" or entry is None or decision.get("inventory_group") and decision["group"].endswith("b"):
                continue
            out += [f"### Ruling {question['number']}, `{question['target']}` {entry['group']}: {', '.join('`' + name + '`' for name in entry['candidates'])}", "",
                    "| Review | Item | Claim's open reason | Exact quote |", "| --- | --- | --- | --- |"]
            seen = set()
            for row in entry["comments"]:
                seen.add((row["review"], row["item_id"], row["quote"]))
                out.append(f"| {short(row['review'])} | {row['item_id']} | {row['open'] or row['outcome']} | \"{cell(row['quote'])}\" |")
            for row in entry["anchors"]:
                if (row["review"], row["item_id"], row["quote"]) not in seen:
                    out.append(f"| {short(row['review'])} | {row['item_id']} | quoted in the candidate record | \"{cell(row['quote'])}\" |")
            out.append("")
    out += ["## The eleven unresolved credit pairs", "", "| Ruling | Pull request | Known problem | Review | Item | The claim's exact words | Grader's two facts | What would settle it, in the grader's words |",
            "| --- | --- | --- | --- | --- | --- | --- | --- |"]
    for question in questions:
        for decision in question["decisions"]:
            entry = by_group.get((question["target"], decision["group"]))
            if decision["decision_type"] == "recovery":
                out.append(f"| {question['number']} | `{question['target']}` {entry['group']} | {entry['family_id']} | {short(entry['review'])} | {entry['item_id']} | \"{cell(entry['quote'])}\" | "
                           f"says what goes wrong: {entry['says_what']}; identifies the cause as a fault: {entry['identifies_cause']} | {cell(entry['would_settle'])} |")
    out += ["", "## Open claims that are not a candidate or a credit pair, by repeated question", "",
            "| Group | Pull request | Claims | Open reason | The question | Kind of need | Settled by, or the prepared question it belongs with |", "| --- | --- | --- | --- | --- | --- | --- |"]
    for group in claims["groups"]:
        where = group["settled_by"] or group["related_question"] or "nothing prepared in this round"
        out.append(f"| {group['id']} | `{group['target']}` | {len(group['members'])} | {group['open']} | {cell(group['question'])} | {group['kind_of_need']} | {cell(where)} |")
    out += ["", "### Their members", "", "| Group | Review | Item | Claim | Exact quote |", "| --- | --- | --- | --- | --- |"]
    for group in claims["groups"]:
        for member in group["members"]:
            out.append(f"| {group['id']} | {short(member['review'])} | {member['item_id']} | {member['claim_id']} | \"{cell(member['quote'])}\" |")
    out += ["", "## Credit claims that are not one of the eleven pairs", "", "| Pull request | Known problem | Review | Item | Exact quote | Why it is not a pair | Belongs with |", "| --- | --- | --- | --- | --- | --- | --- |"]
    for claim in claims["credit_claims"]:
        if not claim["pair"]:
            out.append(f"| `{claim['target']}` | {claim['known_problem']} | {short(claim['review'])} | {claim['item_id']} | \"{cell(claim['quote'])}\" | {cell(claim['why_not_a_pair'])} | {claim['related_question'] or 'nothing prepared in this round'} |")
    (HERE / "round.md").write_text("\n".join(out) + "\n", encoding="utf-8")
    print(len(out), "lines")


if __name__ == "__main__":
    main()
