"""Write this round's candidate before-records (contract 2) into rulings/ from the committed case files, the
recommender's answers in recommender.v1.json and the blind sessions' saved answers.

The recommender's answers were committed before any blind session ran. The before-records need two blind answers
each. The owner's rule is Claude-only, so they can be written only after the owner allows same-family blind answers
for this round in a saved receipt (as `bench/grading/rulings/same-family-blind.v3.md` did for the round before) and
the blind sessions have saved `blind/answers/<folder>/<opus|sonnet>/answers.json`. Until then this script refuses.
Each saved answer must already carry the party fields
(`by`, `model`, `family`, `effort`, `blind`, `exposure`, `brief`, `precedents`, `written`), as the previous round's
`blind/run.py check` added them. The two Astro records share one case file, so one pair of sessions answers both.

    python3 -I docs/research/candidate-rulings-2026-11-07/records.py RECEIPT
    python3 -I docs/research/candidate-rulings-2026-11-07/records.py --draft OUT

RECEIPT is the receipt's repository path (`bench/grading/rulings/same-family-blind.v4.md`); its decision must hold the
passage SCOPE below word for word. --draft
writes recommender-only records into OUT, for checking the fields with `bench-tools ruling check`; a draft is
refused there for lacking blind answers, and is never a before-record. A blind answer drops the fields the record has
no place for and is normalized as `docs/research/owner-rulings-2026-11-04/records.py` does; each record's note lists
the differences. Run from the repository root."""
import hashlib
import json
import os
import re
import sys

R = "docs/research/candidate-rulings-2026-11-07"
ROUND = "candidate-rulings-2026-11-07"
SCOPE = (f"Blind answers from the recommender's model family are allowed in round {ROUND}: one Claude Opus 5.5 and one Claude "
         "Sonnet 5.5 assessor at high effort per case file, without fast mode, at most 14 sessions and $5 at list price in all.")
TERMS = "docs/research/cohort-rebuild-2026-10-05/second-pass/terms"
RULINGS = json.load(open(f"{TERMS}/rulings.index.json"))["rulings"]
CLAUSES = json.load(open(f"{TERMS}/two-questions.v10.clauses.json"))["clauses"]
DROPPED = ("case", "promised", "promised_by", "delivered")


def pin(path):
    return {"path": path, "sha256": hashlib.sha256(open(path, "rb").read()).hexdigest()}


def canonical(name):
    name = re.sub(r"\s*[(:].*$", "", name.strip())
    name = re.sub(r"^Second pass, ruling (\d+)$", r"second-pass \1", name)
    name = re.sub(r"^Last push, ruling (\d+)$", r"last-push \1", name)
    name = re.sub(r"^Ruling ", "", name)
    for candidate in (name, name.split(" ")[0]):
        found = next((key for key, ruling in RULINGS.items() if candidate == key or candidate in ruling["aliases"]), None)
        if found:
            return found
    return None


def blind(source, left_out, normalized, case_name):
    found = next(entry for entry in json.load(open(source)) if entry.get("case") == case_name)
    fields = {key: value for key, value in found.items() if key not in DROPPED}
    fields.setdefault("clauses", [])
    if fields.get("same_fault_as") is None:
        fields.pop("same_fault_as", None)
    gone = [name for name in fields["clauses"] if name not in CLAUSES]
    if gone:
        left_out.append(f"{fields['by']} named {', '.join(gone)} among its clauses; the clause table has no such entry")
    fields["clauses"] = [name for name in fields["clauses"] if name in CLAUSES]
    unknown = [name for name in fields["nearest"] if not canonical(name)]
    if unknown:
        left_out.append(f"{fields['by']} named {', '.join(unknown)} among its nearest; the ruling index has no such entry")
    renamed = [name for name in fields["nearest"] if canonical(name) and canonical(name) != name]
    if renamed:
        left_out.append(f"{fields['by']} named {'; '.join(renamed)} among its nearest, by sheet heading or alias; the record gives their index names")
    fields["nearest"] = [canonical(name) for name in fields["nearest"] if canonical(name)]
    declared = (fields["confidence"], list(fields["short_of_high"]))
    short = list(fields["short_of_high"])
    if not any(RULINGS[name]["reading"] == "current" for name in fields["nearest"]) and "no-precedent" not in short:
        short.append("no-precedent")
    for field, name in (("rule_gap", "gap"), ("conflict", "conflict")):
        if fields.get(field) and name not in short:
            short.append(name)
    fields["short_of_high"] = short
    fields["confidence"] = ("high", "medium", "low")[min(len(short), 2)]
    fields["would_settle"] = fields["confidence"] == "high"
    if (fields["confidence"], short) != declared:
        left_out.append(f"{fields['by']} declared {declared[0]} confidence with the conditions {declared[1] or 'none'}; the record holds {fields['confidence']} with {short}, "
                        "because the tool requires `no-precedent` when no nearest ruling is of the current reading and requires each stated gap or conflict to be named")
        normalized.append({"by": fields["by"], "source": pin(source)})
    return fields


def main(argv):
    draft = argv[:1] == ["--draft"]
    if len(argv) != (2 if draft else 1) or (not draft and not os.path.isfile(argv[0])):
        sys.exit(__doc__)
    out_dir = argv[1] if draft else f"{R}/rulings"
    if not draft and SCOPE not in open(argv[0], encoding="utf-8").read():
        sys.exit(f"refusing: {argv[0]} has no passage reading {SCOPE!r}")
    cands = {c["id"]: c for c in json.load(open("bench/grading/current/candidates.json"))["candidates"]}
    rec = json.load(open(f"{R}/recommender.v1.json"))
    sheet = pin(f"{R}/assessors/precedents-candidate.v1.md")
    os.makedirs(out_dir, exist_ok=True)
    for question in rec["answers"]:
        case = f"{R}/{question['folder']}/{question['case']}"
        for group in question["groups"]:
            path = f"{out_dir}/{question['ruling']}.{group}.before.json"
            if os.path.exists(path):
                sys.exit(f"refusing to replace {path}")
            note = ("Written by records.py from the recommender's answers committed in recommender.v1.json before any blind session, "
                    "and the blind sessions' saved answers.")
            if len(question["groups"]) > 1:
                note += " The records of this question share one case file and one pair of blind sessions."
            record = {
                "contract": 2, "ruling": question["ruling"], "decision_type": "candidate", "target": question["target"], "group": group,
                "reconstructed": False, "note": note,
                "round_opening": pin(f"{R}/opening.json"),
                "filing": {"revision": cands[group]["revision"], "subject": group},
                "dossier": f"{R}/{question['folder']}",
                "case": {**pin(case), "written_by": "claude-opus-5-5 (a preparation agent of the asking session)", "precedents": sheet},
                "rule": pin("bench/rubric/rules.pre-existing.v4.md"),
                "clauses": f"{TERMS}/two-questions.v10.clauses.json",
                "rulings": f"{TERMS}/rulings.index.json",
                "reviews": None,
                "answers": [{"by": "recommender", "model": rec["model"], "family": rec["family"], "effort": rec["effort"],
                             "blind": False, "exposure": rec["exposure"], "brief": None, "precedents": sheet,
                             "written": "before-question", **question["answer"]}],
            }
            if not draft:
                left_out, normalized = [], []
                case_name = os.path.splitext(os.path.basename(question["case"]))[0]
                for model in ("opus", "sonnet"):
                    source = f"{R}/blind/answers/{question['folder']}/{model}/answers.json"
                    record["answers"].append(blind(source, left_out, normalized, case_name))
                if left_out:
                    record["note"] += " Differences between this record and the saved answer files, which keep each party's own words: " + "; ".join(left_out) + "."
                record["same_family_blind"] = {"round": ROUND, "receipt": pin(argv[0]), "scope": SCOPE}
            with open(path, "x") as f:
                f.write(json.dumps(record, indent=1, ensure_ascii=False) + "\n")
            if not draft and normalized:
                with open(path.replace(".before.json", ".declared.json"), "x") as f:
                    f.write(json.dumps({"before": pin(path), "answers": normalized}, indent=1) + "\n")
            print(path)


if __name__ == "__main__":
    main(sys.argv[1:])
