#!/usr/bin/env python3
"""Write the before-record of each decision of one question from the saved first answers.

    python3 docs/research/regrade-rulings-2026-10-13/records.py SPEC.json

SPEC.json names the question and, for each decision, its case file, rule, brief, filing identity and answer files.
The answers are copied as the parties wrote them, with two exceptions that the record's note names each time.
A clause that is not in the clause table and a nearest ruling that is not in the index are left out. Where the tool
requires a condition of high confidence to be named (no nearest ruling of the current reading, a stated gap or a
stated conflict), the condition is added and the level follows the count; the party's own level stays in the saved
answer file, and `<record>.declared.json` lists the answers this touched for the round's `declared-answers.v1.json`."""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
ROUND = "docs/research/regrade-rulings-2026-10-13"
TERMS = "docs/research/cohort-rebuild-2026-10-05/second-pass/terms"
RULES = {"candidate": f"{TERMS}/two-questions.v8.md", "recovery": f"{ROUND}/assessors/rule-credit.md", "band": f"{ROUND}/assessors/rule-labels.md"}
BRIEFS = {"candidate": f"{ROUND}/assessors/brief-candidates.md", "recovery": f"{ROUND}/assessors/brief-credit.md", "band": f"{ROUND}/assessors/brief-labels.md"}
SHEETS = {"candidate": f"{ROUND}/assessors/precedents-candidate.v1.md", "recovery": f"{ROUND}/assessors/precedents-recovery.v1.md", "band": f"{ROUND}/assessors/precedents-band.v1.md"}
PARTIES = {
    "recommender": {"by": "recommender", "model": "claude-opus-5-5", "family": "claude", "effort": "the session's reasoning-effort setting, 30 of 100; no name for it was shown to the session", "blind": False},
    "sol": {"by": "assessor-1", "model": "gpt-6.1-sol", "family": "gpt", "effort": "high", "blind": True, "exposure": "none"},
    "astra": {"by": "assessor-2", "model": "gpt-6-astra", "family": "gpt", "effort": "high", "blind": True, "exposure": "none"},
}
DROPPED = ("case", "promised", "promised_by", "delivered", "against", "sub_kind", "tests")


def load(path):
    return json.loads((ROOT / path).read_text(encoding="utf-8"))


def pin(path):
    return {"path": path, "sha256": hashlib.sha256((ROOT / path).read_bytes()).hexdigest()}


def known(name, rulings):
    return name in rulings or any(name in ruling["aliases"] for ruling in rulings.values())


def answer(party, source, case, decision_type, exposure, sheet, tables, left_out, normalized):
    clauses, rulings = tables
    found = next(entry for entry in load(source) if entry["case"] == case)
    fields = {key: value for key, value in found.items() if key not in DROPPED}
    fields.setdefault("clauses", [])
    for field, keep in (("clauses", lambda name: clauses is None or name in clauses), ("nearest", lambda name: known(name, rulings))):
        gone = [name for name in fields[field] if not keep(name)]
        if gone:
            left_out.append(f"{PARTIES[party]['by']} named {', '.join(gone)} among its {field}; " + ("the clause table" if field == "clauses" else "the ruling index") + " has no such entry")
        fields[field] = [name for name in fields[field] if keep(name)]
    declared = (fields["confidence"], list(fields["short_of_high"]))
    short = list(fields["short_of_high"])
    current = [name for name in fields["nearest"] if next(ruling for key, ruling in rulings.items() if name == key or name in ruling["aliases"])["reading"] == "current"]
    if not current and "no-precedent" not in short:
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
    return {**base, "brief": pin(BRIEFS[decision_type]) if base["blind"] else None, "precedents": sheet, "written": "before-question", **fields}


def main(spec_path):
    spec = json.loads(Path(spec_path).read_text(encoding="utf-8"))
    opening = pin(f"{ROUND}/opening.json")
    for decision in spec["decisions"]:
        kind = decision["decision_type"]
        sheet = pin(SHEETS[kind])
        clauses = f"{TERMS}/two-questions.v8.clauses.json" if kind == "candidate" else None
        tables = (load(clauses)["clauses"] if clauses else None, load(f"{TERMS}/rulings.index.json")["rulings"])
        left_out, normalized = [], []
        answers = [answer(party, source, decision["case_name"], kind, spec["recommender_exposure"], sheet, tables, left_out, normalized) for party, source in decision["answers"].items()]
        note = decision["note"] + (" Differences between this record and the saved answer files, which keep each party's own words: " + "; ".join(left_out) + "." if left_out else "")
        record = {"contract": 2, "ruling": spec["ruling"], "decision_type": kind, "target": spec["target"], "group": decision["group"], "reconstructed": False,
                  "note": note, "round_opening": opening, "filing": decision.get("filing"), "members": decision.get("members"),
                  "dossier": spec["dossier"] if kind == "candidate" else None,
                  "case": {**pin(decision["case"]), "written_by": decision["written_by"], "precedents": sheet},
                  "rule": pin(RULES[kind]), "clauses": clauses, "rulings": f"{TERMS}/rulings.index.json", "reviews": decision.get("reviews"), "answers": answers}
        out = ROOT / spec["out"].format(group=decision["group"])
        out.write_text(json.dumps(record, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
        declared = out.with_name(out.name.replace(".before.json", ".declared.json"))
        if normalized:
            declared.write_text(json.dumps({"before": pin(str(out.relative_to(ROOT))), "answers": normalized}, indent=1) + "\n", encoding="utf-8")
        elif declared.exists():
            declared.unlink()
        print(out.relative_to(ROOT))


if __name__ == "__main__":
    main(sys.argv[1])
