#!/usr/bin/env python3
"""Pin every impact decision to boundary v4 and record the blind inspection under it.

Run from the repository root after the rulings are in `adjudications.json`. Running it again changes nothing."""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "bench/tools"))
import current_grading as current  # noqa: E402

HERE = Path(__file__).resolve().parent
BOUNDARY = HERE / "impact-boundary.v4.md"
INSPECTION = HERE / "independent-impact.v4.json"
CHECKER = ("Fresh Codex GPT-6.1 Sol session at high reasoning effort that saw the cards as committed with boundary v4 "
           "and the boundary v4 rule without its anchors or any label")
INDEPENDENT_OF = "The session that recorded the rulings (Claude Opus 5.5, issue #53)"


def main():
    path = ROOT / "bench/grading/current/adjudications.json"
    record = json.loads(path.read_text(encoding="utf-8"))
    labels = {label["family"]: label for label in json.loads(INSPECTION.read_text(encoding="utf-8"))["labels"]["labels"]}
    source = current.pin_file(INSPECTION, ROOT)
    decisions = [decision for decision in record["decisions"] if decision["dimension"] == "impact"]
    if {decision["subject"] for decision in decisions} != set(labels):
        raise SystemExit("the inspection does not cover exactly the families with an impact decision")
    for decision in decisions:
        label = labels[decision["subject"]]
        decision["boundary"] = current.pin_file(BOUNDARY, ROOT)
        earlier = [check for check in decision["independent_checks"] if check["source"]["path"] != source["path"]]
        decision["independent_checks"] = earlier + [{
            "source": source, "checker": CHECKER, "independent_of": INDEPENDENT_OF,
            "result": "confirmed" if label["band"] == decision["outcome"] else "refuted",
            "reason": f"Under boundary v4, against the user's ruling of {decision['outcome']}. "
                      f"Independent band: {label['band']} ({label['rule']}). {label['reason']}"}]
    path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
