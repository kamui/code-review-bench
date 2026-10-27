#!/usr/bin/env python3
"""Write the before-record of each decision of the round from the saved first answers.

    python3 docs/research/rule-v4-candidates-2026-10-27/records.py [RULING...]

questions.json names each question (a ruling) and, for each of its decisions, the pull request, group, decision
type and the answer files of the recommender and the two blind sessions (one for a label). The answers are copied as
the parties wrote them, with two exceptions that the record's note names each time: a nearest ruling that is not in
the index is left out, and where the tool requires a condition of high confidence to be named (no nearest ruling of
the current reading, a stated gap or a stated conflict) the condition is added and the level follows the count. The
party's own level stays in the saved answer file, and `<record>.declared.json` lists the answers this touched."""
import hashlib
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
ROUND = "docs/research/rule-v4-candidates-2026-10-27"
A = f"{ROUND}/assessors"
TERMS = "docs/research/cohort-rebuild-2026-10-05/second-pass/terms"
KIND = {"candidate": "candidates", "pre-existing-admission": "admission", "recovery": "credit", "pre-existing-raise": "credit", "band": "labels"}
RULES = {"candidate": "bench/rubric/rules.pre-existing.v3.md", "pre-existing-admission": f"{A}/rule-admission.md", "recovery": f"{A}/rule-credit.md",
         "pre-existing-raise": f"{A}/rule-credit.md", "band": f"{A}/rule-labels.md", "band-pre-existing": f"{A}/rule-labels-pre-existing.md"}
BRIEFS = {"candidates": f"{A}/brief-candidates.md", "admission": f"{A}/brief-admission.md", "credit": f"{A}/brief-credit.md", "labels": f"{A}/brief-labels.md"}
SHEETS = {"pre-existing-admission": f"{A}/precedents-pre-existing-admission.v1.md", "recovery": f"{A}/precedents-recovery.v1.md",
          "pre-existing-raise": f"{A}/precedents-pre-existing-raise.v1.md", "band": f"{A}/precedents-band.v1.md"}
CANDIDATE_SHEETS = {}
CANDIDATE_SHEET = f"{A}/precedents-candidate.v1.md"
PARTIES = {
    "recommender": {"by": "recommender", "model": "claude-opus-5-5", "family": "claude", "effort": "the session's reasoning-effort setting; no name for it was shown to the session", "blind": False},
    "sol": {"by": "assessor-1", "model": "gpt-6.1-sol", "family": "gpt", "effort": "high", "blind": True, "exposure": "none"},
    "astra": {"by": "assessor-2", "model": "gpt-6-astra", "family": "gpt", "effort": "high", "blind": True, "exposure": "none"},
}
DROPPED = ("case", "promised", "promised_by", "delivered", "intent", "identifies_instead", "against")


def load(path):
    return json.loads((ROOT / path).read_text(encoding="utf-8"))


def pin(path):
    return {"path": path, "sha256": hashlib.sha256((ROOT / path).read_bytes()).hexdigest()}


def canonical(name, rulings):
    return next((key for key, ruling in rulings.items() if name == key or name in ruling["aliases"]), None)


def answer(party, source, case, brief, exposure, sheet, clauses, rulings, left_out, normalized):
    found = next(entry for entry in load(source) if entry["case"] == case)
    fields = {key: value for key, value in found.items() if key not in DROPPED}
    fields.setdefault("clauses", [])
    if fields.get("same_fault_as") is None:
        fields.pop("same_fault_as", None)
    for field, keep in (("clauses", lambda name: clauses is None or name in clauses), ("nearest", lambda name: canonical(name, rulings))):
        gone = [name for name in fields[field] if not keep(name)]
        if gone:
            left_out.append(f"{PARTIES[party]['by']} named {', '.join(gone)} among its {field}; " + ("the clause table" if field == "clauses" else "the ruling index") + " has no such entry")
        fields[field] = [name for name in fields[field] if keep(name)]
    declared = (fields["confidence"], list(fields["short_of_high"]))
    short = list(fields["short_of_high"])
    if not any(rulings[canonical(name, rulings)]["reading"] == "current" for name in fields["nearest"]) and "no-precedent" not in short:
        short.append("no-precedent")
    for field, name in (("rule_gap", "gap"), ("conflict", "conflict")):
        if fields.get(field) and name not in short:
            short.append(name)
    fields["short_of_high"] = short
    fields["confidence"] = ("high", "medium", "low")[min(len(short), 2)]
    fields["would_settle"] = fields["confidence"] == "high"
    if (fields["confidence"], short) != declared:
        left_out.append(f"{PARTIES[party]['by']} declared {declared[0]} confidence with the conditions {declared[1] or 'none'}; the record holds {fields['confidence']} with {short}, "
                        "because the tool requires `no-precedent` when no nearest ruling is of the current reading and requires each stated gap or conflict to be named")
        normalized.append({"by": PARTIES[party]["by"], "source": pin(source), "case": case})
    base = dict(PARTIES[party])
    base.setdefault("exposure", exposure)
    return {**base, "brief": pin(brief) if base["blind"] else None, "precedents": sheet, "written": "before-question", **fields}


def main(only):
    spec = load(f"{ROUND}/questions.json")
    opening = pin(f"{ROUND}/opening.json")
    rulings = load(f"{TERMS}/rulings.index.json")["rulings"]
    for question in spec["questions"]:
        if only and question["ruling"] not in only:
            continue
        for decision in question["decisions"]:
            kind = decision["decision_type"]
            rule = RULES["band-pre-existing" if kind == "band" and decision.get("pre_existing") else kind]
            sheet = pin(CANDIDATE_SHEETS.get(decision["folder"], CANDIDATE_SHEET) if kind == "candidate" else SHEETS[kind])
            clauses_path = f"{TERMS}/two-questions.v10.clauses.json" if kind == "candidate" else None
            clauses = load(clauses_path)["clauses"] if clauses_path else None
            folder, kindname = f"{A}/{decision['folder']}", KIND[kind]
            files = {"recommender": f"{folder}/recommender-{kindname}.json"}
            for model in decision.get("blind", ("sol", "astra")):
                files[model] = f"{folder}/answers-{kindname}-{model}.json"
            left_out, normalized = [], []
            answers = [answer(party, source, decision["case_name"], BRIEFS[kindname], decision.get("recommender_exposure", "none"), sheet, clauses, rulings, left_out, normalized)
                       for party, source in files.items()]
            note = decision.get("note", "") + (" Differences between this record and the saved answer files, which keep each party's own words: " + "; ".join(left_out) + "." if left_out else "")
            source = f"{ROUND}/{decision['target']}/{'cards' if kind == 'band' else 'cases'}/{decision['group']}.md"
            record = {"contract": 2, "ruling": question["ruling"], "decision_type": kind, "target": decision["target"], "group": decision["group"], "reconstructed": False,
                      "note": note.strip(), "round_opening": opening, "filing": decision.get("filing"),
                      "dossier": f"{ROUND}/{decision['target']}" if kind == "candidate" else None,
                      "case": {**pin(source), "written_by": "gpt-6.1-sol (a preparation agent of the asking session)", "precedents": sheet},
                      "rule": pin(rule), "clauses": clauses_path, "rulings": f"{TERMS}/rulings.index.json", "reviews": decision.get("reviews"), "answers": answers}
            out = HERE / "rulings" / f"{question['ruling']}.{decision['group']}{'-label' if kind == 'band' else ''}.before.json"
            out.write_text(json.dumps(record, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
            declared = out.with_name(out.name.replace(".before.json", ".declared.json"))
            if normalized:
                declared.write_text(json.dumps({"before": pin(str(out.relative_to(ROOT))), "answers": normalized}, indent=1) + "\n", encoding="utf-8")
            elif declared.exists():
                declared.unlink()
            print(out.relative_to(ROOT))


if __name__ == "__main__":
    main(sys.argv[1:])
