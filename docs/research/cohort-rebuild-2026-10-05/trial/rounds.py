#!/usr/bin/env python3
"""Compare each grader with its own answers in an earlier round and write the claims it now answers differently.

    python3 docs/research/cohort-rebuild-2026-10-05/trial/rounds.py [BATCHES EARLIER LATER OUT]

Both rounds labelled the same lists of claims (`run.py --fixed-claims`), so claims pair by review, item and position.
For each grader it reports how many labels and how many answers to the two facts stayed the same, the derived result
for every review and known problem in both rounds, and every claim whose label or facts changed with both rounds'
reasons. A fact with no entry for a known problem counts as "no". Run from the repository root."""
import json
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "bench/tools"))
import claim_grading  # noqa: E402

HERE = Path(__file__).resolve().parent
FACTS = ("says_what", "says_why")


def load(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def by_attempt(directory, run):
    tokens, verdicts = load(directory / "tokens.json"), load(directory / "verdicts.json")
    return {f"{run}/{tokens[token]}": review for token, review in verdicts["reviews"].items()}


def entry(claim, family):
    return next((e for e in claim["known_problems"] if e["family"] == family), {"says_what": "no", "says_why": "no", "reason": None})


def recovery(family, review, admitted):
    claims = [{"id": c["id"], "known_problems": c["known_problems"]} for item in review["items"].values() for c in item["claims"]]
    return claim_grading.family_recovery_v2(family, claims, admitted)[0]


def main(batches_file="batches.json", earlier="results", later="retest-section-3", out="retest-section-3-rounds.json"):
    batches = load(HERE / batches_file)["batches"]
    references = {r["target"]: r["families"] for r in load(ROOT / ".local/trial/current/references.json")["targets"]}
    admitted = {a["id"]: a["admission"]["state"] == "admitted" for a in load(ROOT / ".local/trial/current/inventory.json")["attempts"]}
    report = {}
    for stage in ("first", "second"):
        labels, facts, results, changed, shifted = Counter(), {fact: Counter() for fact in FACTS}, Counter(), [], []
        for batch in batches:
            run = Path(batch["run"]).name
            old, new = (by_attempt(HERE / directory / run / batch["target"] / stage, run) for directory in (earlier, later))
            for attempt, review in old.items():
                for number, item in review["items"].items():
                    pairs = list(zip(item["claims"], new[attempt]["items"][number]["claims"], strict=True))
                    for index, (a, b) in enumerate(pairs):
                        labels[a["outcome"] == b["outcome"]] += 1
                        families = sorted({e["family"] for e in a["known_problems"]} | {e["family"] for e in b["known_problems"]})
                        moved = []
                        for family in families:
                            before, after = entry(a, family), entry(b, family)
                            for fact in FACTS:
                                facts[fact][before[fact] == after[fact]] += 1
                            if any(before[fact] != after[fact] for fact in FACTS):
                                moved.append({"family": family, "earlier": before, "later": after})
                        if moved or a["outcome"] != b["outcome"]:
                            changed.append({"attempt": attempt, "item": int(number), "claim": index + 1, "quote": a["quote"],
                                            "earlier": {k: a[k] for k in ("outcome", "kind", "notes")},
                                            "later": {k: b[k] for k in ("outcome", "kind", "notes")}, "known_problems": moved})
                for family in references[batch["target"]]:
                    before, after = recovery(family, review, admitted[attempt]), recovery(family, new[attempt], admitted[attempt])
                    results[(before, after)] += 1
                    if before != after:
                        shifted.append({"attempt": attempt, "family": family["id"], "earlier": before, "later": after})
        report[stage] = {"labels": {"same": labels[True], "total": sum(labels.values())},
                         "facts": {fact: {"same": facts[fact][True], "total": sum(facts[fact].values())} for fact in FACTS},
                         "known_problem_results": {"same": sum(n for (a, b), n in results.items() if a == b), "total": sum(results.values()),
                                                   "changed": shifted},
                         "changed_claims": changed}
        print(stage, "labels", labels[True], "of", sum(labels.values()), "|", *(f"{fact} {facts[fact][True]} of {sum(facts[fact].values())}" for fact in FACTS),
              "| results", report[stage]["known_problem_results"]["same"], "of", sum(results.values()), "|", len(changed), "claims changed")
    (HERE / out).write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main(*sys.argv[1:])
