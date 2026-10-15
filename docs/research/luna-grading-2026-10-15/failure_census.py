"""Count classifier failures before changing the response contract."""

from collections import Counter
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
BASE = ROOT.parent / "jev-grading-2026-10-15"
result = {"premise_under_test": "Requiring generated exact quotations is necessary to evaluate bounded classification.",
          "actors": {}}
for actor, paths in (
    ("sol", (BASE / "results").glob("sol-*/**/receipt.json")),
    ("luna", (ROOT / "results").glob("luna-*/**/receipt.json")),
    ("jev", (BASE / "results").glob("jev-*/*/result.json")),
):
    receipts = [json.loads(path.read_text()) for path in paths]
    result["actors"][actor] = {
        "attempts": len(receipts),
        "completed": sum(row["status"] == "completed" for row in receipts),
        "failures": dict(Counter(row.get("error", row.get("failure_type", "unknown"))
                                 for row in receipts if row["status"] != "completed")),
        "generated_quotations_required": actor != "jev",
    }
print(json.dumps(result, indent=2))
