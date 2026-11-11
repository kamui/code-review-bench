"""Write the after-records of the owner's answers of 2026-11-11 (answers.md) beside their before-records in rulings/.
Each one pins its before-record. The owner gave no reason beyond the chosen option, so `ground` is null; each
decision is the recommender's first answer, asked once, so `miss` is null. An after-record that exists is not
replaced. Run from the repository root:

    python3 -I docs/research/candidate-rulings-2026-11-10/after.py"""
import hashlib
import json
from pathlib import Path

R = Path("docs/research/candidate-rulings-2026-11-10/rulings")
DECIDED = {
    "01-django-gt-v16-stale-wrapper-rebuild.NC-0266123a5aa5": ("duplicate", "GT-v16"),
    "02-django-pool-copy-edited-settings.N1b": ("suggestion", None),
}
for stem, (outcome, same) in DECIDED.items():
    before = R / f"{stem}.before.json"
    record = json.loads(before.read_text(encoding="utf-8"))
    recommender = next(answer for answer in record["answers"] if answer["by"] == "recommender")
    assert (recommender["outcome"], recommender.get("same_fault_as")) == (outcome, same), stem
    after = {"contract": 2, "ruling": record["ruling"],
             "before": {"path": str(before), "sha256": hashlib.sha256(before.read_bytes()).hexdigest()},
             "settled_by": "owner", "policy": None, "asked": 1, "outcome": outcome, "by_default": False,
             "ground": None, "miss": None, "lesson": None, "same_fault_as": same}
    with open(R / f"{stem}.after.json", "x", encoding="utf-8") as f:
        f.write(json.dumps(after, indent=1) + "\n")
    print(R / f"{stem}.after.json")
