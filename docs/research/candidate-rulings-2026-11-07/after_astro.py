"""Write the after-records of the owner's answer of 2026-11-08 on question 3 (answers.md entry 10) beside the two
Astro before-records in rulings/. The decision is the recommender's first answer, unproven, but the owner was asked
three times and the session's second recommendation was wrong, so `miss` is a slip. An after-record that exists is
not replaced. Run from the repository root:

    python3 -I docs/research/candidate-rulings-2026-11-07/after_astro.py"""
import hashlib
import json
from pathlib import Path

R = Path("docs/research/candidate-rulings-2026-11-07/rulings")
GROUND = "The limit is key here. If we dont know this was the case when the PR was posted, we cna't say it's actually false can we?"
MISS = {"cause": "slip",
        "note": "After the 2026-11-08 Vercel run the session recommended refuted from evidence after the cut-off, which ground rule 3 bars as a reason, and the owner pointed this out."}
for stem in ["03-astro-visitor-x-astro-path.NC-de9bfe6de5b8", "03-astro-visitor-x-astro-path.NC-a0a4cf9ad59c"]:
    before = R / f"{stem}.before.json"
    record = json.loads(before.read_text(encoding="utf-8"))
    recommender = next(answer for answer in record["answers"] if answer["by"] == "recommender")
    assert recommender["outcome"] == "unproven", stem
    after = {"contract": 2, "ruling": record["ruling"],
             "before": {"path": str(before), "sha256": hashlib.sha256(before.read_bytes()).hexdigest()},
             "settled_by": "owner", "policy": None, "asked": 3, "outcome": "unproven", "by_default": False,
             "ground": GROUND, "miss": MISS, "lesson": None}
    with open(R / f"{stem}.after.json", "x", encoding="utf-8") as f:
        f.write(json.dumps(after, indent=1) + "\n")
    print(R / f"{stem}.after.json")
