#!/usr/bin/env python3
"""Compare four graders that labelled the same lists of claims and write `comparison.json`.

    python3 docs/research/grader-model-test-2026-10-12/compare.py --old-state DIR

The graders are the trial's saved Claude Opus 5.5 and Codex GPT-6.1 Sol sessions and this test's Claude Sonnet 5.5
and Claude Haiku 5.5 sessions. Every grader labelled the list of claims `batches.json` names, so claims pair by
review, item and position, and a pair whose quotes differ stops the comparison. The counting is the trial's
`compare.py`: a claim's label; the label for claims neither grader tied to a saved ruling; "says what goes wrong"
for every claim and known problem either grader recorded, where no entry counts as "no"; and caught, missed or
unresolved for every review and known problem, derived by the `claim_grading.py` of DIR, the checkout the sessions
were prepared from, from the answer key and admissions of its `.local/trial/current`. The second fact, "identifies
the cause as a fault", is not compared (decision P24). It stops unless Opus against Sol gives the figures the trial
published. Run from the repository root."""
import argparse
import importlib.util
import json
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
TRIAL = "docs/research/cohort-rebuild-2026-10-05/trial"
GRADERS = ("opus", "sol", "sonnet", "haiku")
NAMES = {"opus": "Claude Opus 5.5", "sol": "Codex GPT-6.1 Sol", "sonnet": "Claude Sonnet 5.5", "haiku": "Claude Haiku 5.5"}
PAIRS = (("opus", "sol"), ("sonnet", "sol"), ("haiku", "sol"), ("sonnet", "opus"), ("haiku", "opus"), ("sonnet", "haiku"))
SETS = ("rehearsed", "untouched")
MEASURES = ("labels", "labels_with_saved_ruling", "labels_without_saved_ruling", "says_what", "says_what_with_saved_ruling",
            "says_what_without_saved_ruling", "credit")
ANSWERS = ("true", "this_change", "promised", "delivered")
# labels, labels without a saved ruling, says_what, credit: retest-final-comparison.json and retest-final-untouched-comparison.json
PUBLISHED = {"rehearsed": ((401, 422), (298, 319), (444, 450), (394, 397)), "untouched": ((59, 71), (47, 59), (43, 44), (33, 33))}


def saved(batch, grader):
    name = f"{Path(batch['run']).name}/{batch['target']}"
    return {"opus": ROOT / TRIAL / batch["opus"], "sol": ROOT / TRIAL / batch["sol"],
            "sonnet": HERE / "results/claude-sonnet-5-5" / name, "haiku": HERE / "results/claude-haiku-5-5" / name}[grader]


def table(counter):
    return {"agree": sum(n for (a, b), n in counter.items() if a == b), "total": sum(counter.values()),
            "pairs": [{"first": a, "second": b, "count": n} for (a, b), n in sorted(counter.items(), key=lambda kv: (-kv[1], kv[0]))]}


def behind(review, family):
    """The claims of a review that record something other than "no" on "says what goes wrong" for a known problem."""
    return [{"item": int(number), "claim": index + 1, "quote": claim["quote"], "says_what": entry["says_what"], "reason": entry["reason"]}
            for number, item in review["items"].items() for index, claim in enumerate(item["claims"])
            for entry in claim["known_problems"] if entry["family"] == family and entry["says_what"] != "no"]


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--old-state", required=True)
    old = Path(parser.parse_args().old_state).resolve()
    spec = importlib.util.spec_from_file_location("trial_compare", old / TRIAL / "compare.py")
    trial = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(trial)
    references = {r["target"]: r["families"] for r in trial.load(old / ".local/trial/current/references.json")["targets"]}
    admitted = {a["id"]: a["admission"]["state"] == "admitted" for a in trial.load(old / ".local/trial/current/inventory.json")["attempts"]}
    counts = {(pair, group, measure): Counter() for pair in PAIRS for group in SETS for measure in MEASURES}
    outcomes = {(grader, group): Counter() for grader in GRADERS for group in SETS}
    credits = {(grader, group): Counter() for grader in GRADERS for group in SETS}
    extras = {grader: Counter() for grader in GRADERS}
    present = {grader: 0 for grader in GRADERS}
    credit_differences, label_differences, reviews_of = [], [], {}
    for batch in trial.load(HERE / "batches.json")["batches"]:
        group, run = batch["set"], Path(batch["run"]).name
        graded = {grader: trial.by_attempt(saved(batch, grader), run) for grader in GRADERS if (saved(batch, grader) / "verdicts.json").exists()}
        for grader in graded:
            present[grader] += 1
            verdicts = trial.load(saved(batch, grader) / "verdicts.json")
            extras[grader]["new_candidates"] += len(verdicts["new_candidates"])
            extras[grader]["link_disputes"] += len(verdicts["link_disputes"])
        for attempt, review in graded["opus"].items():
            reviews_of[attempt] = {grader: reviews[attempt] for grader, reviews in graded.items()}
            for number, item in review["items"].items():
                claims = {grader: reviews[attempt]["items"][number]["claims"] for grader, reviews in graded.items()}
                for index, claim in enumerate(item["claims"]):
                    answers = {grader: listed[index] for grader, listed in claims.items()}
                    if any(len(listed) != len(item["claims"]) or answer["quote"] != claim["quote"] for listed, answer in zip(claims.values(), answers.values())):
                        raise SystemExit(f"{attempt} item {number} claim {index + 1}: the graders did not label the same claim")
                    for grader, answer in answers.items():
                        outcomes[(grader, group)][answer["outcome"]] += 1
                        extras[grader]["claims_tied_to_a_saved_ruling"] += answer["canonical_claim_id"] is not None
                        if answer["outcome"] == "unresolved":
                            extras[grader][f"unresolved: {answer['open']['kind']}"] += 1
                    for pair in PAIRS:
                        if not set(pair) <= set(answers):
                            continue
                        a, b = answers[pair[0]], answers[pair[1]]
                        counts[(pair, group, "labels")][(a["outcome"], b["outcome"])] += 1
                        # A claim either grader tied to a saved ruling carries that ruling's label, so it cannot disagree.
                        ruled = a["canonical_claim_id"] is not None or b["canonical_claim_id"] is not None
                        counts[(pair, group, "labels_with_saved_ruling" if ruled else "labels_without_saved_ruling")][(a["outcome"], b["outcome"])] += 1
                        for family in {e["family"] for e in a["known_problems"]} | {e["family"] for e in b["known_problems"]}:
                            pick = lambda answer: next((e["says_what"] for e in answer["known_problems"] if e["family"] == family), "no")
                            counts[(pair, group, "says_what")][(pick(a), pick(b))] += 1
                            counts[(pair, group, "says_what_with_saved_ruling" if ruled else "says_what_without_saved_ruling")][(pick(a), pick(b))] += 1
                    if len({answer["outcome"] for answer in answers.values()}) > 1:
                        label_differences.append({
                            "set": group, "target": batch["target"], "attempt": attempt, "item": int(number), "claim": index + 1, "quote": claim["quote"],
                            **{grader: {k: answer[k] for k in (*ANSWERS, "outcome", "kind", "open", "canonical_claim_id", "notes")}
                               | {"says_what": {e["family"]: e["says_what"] for e in answer["known_problems"]}}
                               for grader, answer in answers.items()}})
            for family in references[batch["target"]]:
                derived = {grader: trial.claim_grading.family_recovery_v2(
                    family, [{"id": c["id"], "known_problems": c["known_problems"]} for item in reviews[attempt]["items"].values() for c in item["claims"]],
                    admitted[attempt])[0] for grader, reviews in graded.items()}
                for grader, result in derived.items():
                    credits[(grader, group)][result] += 1
                for pair in PAIRS:
                    if set(pair) <= set(derived):
                        counts[(pair, group, "credit")][(derived[pair[0]], derived[pair[1]])] += 1
                if len(set(derived.values())) > 1:
                    credit_differences.append({"set": group, "target": batch["target"], "attempt": attempt, "known_problem": family["id"],
                                               "title": family["title"], "admitted": admitted[attempt], **derived,
                                               "claims": {grader: behind(reviews[attempt], family["id"]) for grader, reviews in graded.items()}})

    for group, published in PUBLISHED.items():
        ours = tuple((table(counts[(("opus", "sol"), group, measure)])["agree"], table(counts[(("opus", "sol"), group, measure)])["total"])
                     for measure in ("labels", "labels_without_saved_ruling", "says_what", "credit"))
        if ours != published:
            raise SystemExit(f"Opus against Sol on the {group} batches gives {ours}, and the trial published {published}")
    print("Opus against Sol gives the trial's published figures on both sets of batches\n")

    # The owner's rulings on single comments, which no grader was shown: the first fact over the item's claims.
    ruled = [{**{k: comment[k] for k in ("target", "family", "attempt", "item", "ruling")}, "ruled_says_what": "yes" if comment["says_what"] else "no",
              **{grader: trial.facts(review, comment["item"], comment["family"])["says_what"] for grader, review in reviews_of[comment["attempt"]].items()}}
             for comment in trial.load(ROOT / TRIAL / "ruled-comments.json")["comments"]]
    comparison = {"batches_graded": present, "ruled_comments": ruled,
                  "pairs": {f"{a} against {b}": {group: {measure: table(counts[((a, b), group, measure)]) for measure in MEASURES} for group in SETS}
                            for a, b in PAIRS if present[a] and present[b]},
                  "outcomes": {grader: {group: dict(outcomes[(grader, group)].most_common()) for group in SETS} for grader in GRADERS if present[grader]},
                  "credit": {grader: {group: dict(credits[(grader, group)].most_common()) for group in SETS} for grader in GRADERS if present[grader]},
                  "raised": {grader: {k: v for k, v in extras[grader].items() if v} for grader in GRADERS if present[grader]},
                  "credit_differences": credit_differences, "label_differences": label_differences}
    (HERE / "comparison.json").write_text(json.dumps(comparison, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")

    cell = lambda pair, group, measure: "{agree} of {total}".format(**table(counts[(pair, group, measure)]))
    both = lambda pair, measure: "{agree} of {total}".format(**table(counts[(pair, "rehearsed", measure)] + counts[(pair, "untouched", measure)]))
    for measure in MEASURES:
        print(f"{measure}\n\n| Compared | Ten rehearsed batches | Two untouched batches | All twelve |\n| --- | ---: | ---: | ---: |")
        for pair in PAIRS:
            if present[pair[0]] and present[pair[1]]:
                print(f"| {NAMES[pair[0]]} against {NAMES[pair[1]]} | {cell(pair, 'rehearsed', measure)} | {cell(pair, 'untouched', measure)} | {both(pair, measure)} |")
        print()
    for name, tally in (("outcomes", outcomes), ("credit", credits)):
        columns = [grader for grader in GRADERS if present[grader]]
        total = {grader: tally[(grader, "rehearsed")] + tally[(grader, "untouched")] for grader in columns}
        print(f"{name}\n\n| Result | " + " | ".join(NAMES[g] for g in columns) + " |\n| --- | " + " | ".join("---:" for _ in columns) + " |")
        for value in sorted({v for counter in total.values() for v in counter}, key=lambda v: -total[columns[0]][v]):
            print(f"| {value} | " + " | ".join(str(total[g][value]) for g in columns) + " |")
        print()
    for pair in PAIRS:
        if present[pair[0]] and present[pair[1]]:
            mixed = counts[(pair, "rehearsed", "labels")] + counts[(pair, "untouched", "labels")]
            print(f"{pair[0]} against {pair[1]}, labels that differ:", ", ".join(f"{a} / {b} {n}" for (a, b), n in mixed.most_common() if a != b))
    print(f"\nthe owner's rulings on {len(ruled)} comments, same answer on the first fact:",
          ", ".join(f"{NAMES[g]} {sum(row.get(g) == row['ruled_says_what'] for row in ruled)}" for g in GRADERS if present[g]))
    for row in ruled:
        if any(row.get(g) != row["ruled_says_what"] for g in GRADERS if present[g]):
            print(" ", row["attempt"], "item", row["item"], row["family"], row["ruling"], "ruled", row["ruled_says_what"], {g: row[g] for g in GRADERS if g in row})
    print({grader: dict(extras[grader]) for grader in GRADERS})
    print(f"\n{len(credit_differences)} reviews and known problems where the graders' credit differs, {len(label_differences)} claims where their labels differ")
    for row in credit_differences:
        print(" ", row["set"], row["attempt"], row["known_problem"], {grader: row[grader] for grader in GRADERS if grader in row})


if __name__ == "__main__":
    main()
