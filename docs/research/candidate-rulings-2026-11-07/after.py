"""Write the after-records of the owner's answers of 2026-11-07 (answers.md) beside their before-records in rulings/.
Each one pins its before-record. The owner gave no reason beyond the chosen option, so `ground` is null; each
decision is the recommender's first answer, asked once, so `miss` is null. The two Astro records stay pending: the
owner chose "Test on Vercel first". An after-record that exists is not replaced. Run from the repository root:

    python3 -I docs/research/candidate-rulings-2026-11-07/after.py"""
import hashlib
import json
from pathlib import Path

R = Path("docs/research/candidate-rulings-2026-11-07/rulings")
DECIDED = {
    "01-requests-gt-i2-empty-bundle.NC-7b18f986a6f0": ("duplicate", "GT-i2"),
    "02-ripgrep-sourced-after-compdef.NC-896d9ea6c530": ("refuted", None),
    "04-hono-json-space-before-semicolon.NC-6f7e0c93f8ec": ("problem", None),
    "05-base-ui-first-keystroke-render.NC-cf43c4347d82": ("suggestion", None),
    "06-django-pool-health-checks.NC-fa83fdbcea6f": ("suggestion", None),
}
for stem, (outcome, same) in DECIDED.items():
    before = R / f"{stem}.before.json"
    record = json.loads(before.read_text(encoding="utf-8"))
    recommender = next(answer for answer in record["answers"] if answer["by"] == "recommender")
    assert (recommender["outcome"], recommender.get("same_fault_as")) == (outcome, same), stem
    after = {"contract": 2, "ruling": record["ruling"],
             "before": {"path": str(before), "sha256": hashlib.sha256(before.read_bytes()).hexdigest()},
             "settled_by": "owner", "policy": None, "asked": 1, "outcome": outcome, "by_default": False,
             "ground": None, "miss": None, "lesson": None}
    if same:
        after["same_fault_as"] = same
    with open(R / f"{stem}.after.json", "x", encoding="utf-8") as f:
        f.write(json.dumps(after, indent=1) + "\n")
    print(R / f"{stem}.after.json")
