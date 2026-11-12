"""Write the label (`band`) before-record for the new Hono problem from candidate NC-6f7e0c93f8ec, which the owner
ruled a new problem on 2026-11-07 (answers.md, question 4).

A band record needs one independent inspector, the blind label session of `docs/impact-calibration.md`, "Assign an
impact band", step 3, and a `serious` band needs that inspector's confirming check. The inspector would be Claude, and
`bench/grading/rulings/same-family-blind.v4.md` allows same-family answers for this round's candidate records only.
So this script refuses until a new owner receipt allows a same-family label inspector for this round and the
inspector's answer is saved at blind/answers/labels/<model>/answers.json with the party fields added.

    python3 -I docs/research/candidate-rulings-2026-11-07/labels.py RECEIPT SCOPE
    python3 -I docs/research/candidate-rulings-2026-11-07/labels.py --draft OUT

SCOPE is the receipt's passage, word for word, that allows it for this round. --draft writes a recommender-only
record into OUT, for checking the fields with `bench-tools ruling check`; it is never a before-record. Run from the
repository root."""
import hashlib
import json
import os
import sys

R = "docs/research/candidate-rulings-2026-11-07"
ROUND = "candidate-rulings-2026-11-07"
INSPECTORS = ("opus",)


def pin(path):
    return {"path": path, "sha256": hashlib.sha256(open(path, "rb").read()).hexdigest()}


def main(argv):
    draft = argv[:1] == ["--draft"]
    if len(argv) != 2 or (not draft and not os.path.isfile(argv[0])):
        sys.exit(__doc__)
    if not draft and argv[1] not in open(argv[0], encoding="utf-8").read():
        sys.exit(f"refusing: {argv[0]} has no passage reading {argv[1]!r}")
    out_dir = argv[1] if draft else f"{R}/rulings"
    rec = json.load(open(f"{R}/recommender-labels.v1.json"))
    sheet = pin(f"{R}/assessors/precedents-band.v1.md")
    os.makedirs(out_dir, exist_ok=True)
    for question in rec["answers"]:
        path = f"{out_dir}/{question['ruling']}.{question['group']}-label.before.json"
        if os.path.exists(path):
            sys.exit(f"refusing to replace {path}")
        record = {
            "contract": 2, "ruling": question["ruling"], "decision_type": "band", "target": question["target"],
            "group": question["group"], "reconstructed": False,
            "note": "Written by labels.py from the recommender's label answer in recommender-labels.v1.json and the blind inspector's saved answer.",
            "round_opening": pin(f"{R}/opening.json"), "filing": None, "dossier": None,
            "case": {**pin(f"{R}/{question['card']}"), "written_by": "claude-opus-5-5 (a preparation agent of the asking session)", "precedents": sheet},
            "rule": pin(f"{R}/assessors/rule-labels.md"), "clauses": None,
            "rulings": "docs/research/cohort-rebuild-2026-10-05/second-pass/terms/rulings.index.json", "reviews": None,
            "answers": [{"by": "recommender", "model": rec["model"], "family": rec["family"], "effort": rec["effort"], "blind": False,
                         "exposure": rec["exposure"], "brief": None, "precedents": sheet, "written": "before-question", **question["answer"]}],
        }
        if not draft:
            for model in INSPECTORS:
                found = json.load(open(f"{R}/blind/answers/labels/{model}/answers.json"))[0]
                record["answers"].append({key: value for key, value in found.items() if key not in ("case", "against")})
            record["same_family_blind"] = {"round": ROUND, "receipt": pin(argv[0]), "scope": argv[1]}
        with open(path, "x", encoding="utf-8") as f:
            f.write(json.dumps(record, indent=1, ensure_ascii=False) + "\n")
        print(path)


if __name__ == "__main__":
    main(sys.argv[1:])
