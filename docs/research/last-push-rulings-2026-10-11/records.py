#!/usr/bin/env python3
"""Write the before-record of each decision of one question from the saved first answers.

    python3 docs/research/last-push-rulings-2026-10-11/records.py SPEC.json

SPEC.json names the question and, for each decision, its case file, rule, saved ruling and the answer files.
The answers are copied as the parties wrote them. A ruling an assessor named by an alias keeps that name;
`bench/tools/ruling_record.py` resolves it."""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
TERMS = "docs/research/cohort-rebuild-2026-10-05/second-pass/terms"
PARTIES = {
    "recommender": {"model": "claude-opus-5-5", "family": "claude", "effort": "high", "blind": False},
    "sol": {"by": "assessor-1", "model": "gpt-6.1-sol", "family": "gpt", "effort": "high", "blind": True, "exposure": "none"},
    "astra": {"by": "assessor-2", "model": "gpt-6-astra", "family": "gpt", "effort": "high", "blind": True, "exposure": "none"},
}


def pin(path):
    return {"path": path, "sha256": hashlib.sha256((ROOT / path).read_bytes()).hexdigest()}


def answer(party, source, case, exposure, brief, overrides):
    found = next(entry for entry in json.loads((ROOT / source).read_text(encoding="utf-8")) if entry["case"] == case)
    fields = {key: value for key, value in found.items() if key not in ("case", "promised", "promised_by", "delivered", "against", "sub_kind", "tests")}
    fields.setdefault("clauses", [])
    base = dict(PARTIES[party])
    base.setdefault("by", party)
    base.setdefault("exposure", exposure)
    return {**base, "brief": pin(brief) if base["blind"] else None, "written": "before-question", **fields, **overrides}


def main(spec_path):
    spec = json.loads(Path(spec_path).read_text(encoding="utf-8"))
    for decision in spec["decisions"]:
        candidate = decision["decision_type"] == "candidate"
        record = {
            "contract": 2,
            "ruling": spec["ruling"],
            "decision_type": decision["decision_type"],
            "target": spec["target"],
            "group": decision["group"],
            "reconstructed": False,
            "note": decision["note"],
            "dossier": spec["dossier"] if candidate else None,
            "case": {**pin(decision["case"]), "written_by": decision["written_by"], "precedents": None},
            "rule": pin(decision["rule"]),
            "clauses": f"{TERMS}/two-questions.v7.clauses.json" if candidate else None,
            "rulings": f"{TERMS}/rulings.index.json",
            "reviews": decision["reviews"],
            "answers": [answer(party, source, decision["case_name"], spec["recommender_exposure"], decision["brief"], decision.get("overrides", {}).get(party, {}))
                        for party, source in decision["answers"].items()],
        }
        out = ROOT / spec["out"].format(group=decision["group"])
        out.write_text(json.dumps(record, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
        print(out.relative_to(ROOT))


if __name__ == "__main__":
    main(sys.argv[1])
