"""Collect the grader text that says a check was refused or could not be run, per target."""
import collections
import glob
import json
import re
import sys

PATTERN = re.compile(
    r"refus|allowance|could not (?:be )?run|cannot (?:be )?run|couldn't run|could not execute|"
    r"cannot execute|not (?:permitted|available|provisioned)|unavailable|no encoded|blocked|"
    r"could not (?:check|verify|confirm|test|build|compile|type-?check)|without (?:running|executing)|"
    r"not run|wasn't run|was not run|denied",
    re.I,
)


def texts(claim):
    for key in ("notes", "reason"):
        value = claim.get(key)
        if isinstance(value, str):
            yield key, value
    for value in claim.get("evidence") or []:
        if isinstance(value, str):
            yield "evidence", value
    for known in claim.get("known_problems") or []:
        if isinstance(known, dict) and isinstance(known.get("reason"), str):
            yield "known.reason", known["reason"]
    opened = claim.get("open")
    if isinstance(opened, dict):
        for key, value in opened.items():
            if isinstance(value, str):
                yield f"open.{key}", value


def scan(path, source, out, counts):
    data = json.load(open(path))
    target = path.split("/")[-3]
    for blind, review in (data.get("reviews") or {}).items():
        for number, item in (review.get("items") or {}).items():
            for claim in item.get("claims") or []:
                hits = [(k, v) for k, v in texts(claim) if PATTERN.search(v)]
                if hits:
                    counts[(source, target)] += 1
                    out[target].append(
                        {
                            "source": source,
                            "path": path,
                            "claim": f"{blind}/{number}/{claim.get('id')}",
                            "outcome": claim.get("outcome"),
                            "quote": (claim.get("quote") or "")[:300],
                            "hits": [f"{k}: {v[:700]}" for k, v in hits],
                        }
                    )


def main():
    grades = json.load(open("bench/grading/current/grades.json"))
    out = collections.defaultdict(list)
    counts = collections.Counter()
    for batch in grades["batches"]:
        verdicts = batch["assessor"]["verdicts"]["path"]
        scan(verdicts, "first", out, counts)
    for path in sorted(
        glob.glob("bench/grading/current/audit/evaluator-audit-v2-2026-10-12/second/*/*/assessment-*/verdicts.json")
    ):
        scan(path, "second", out, counts)
    directory = sys.argv[1]
    for target, rows in out.items():
        with open(f"{directory}/{target}.json", "w") as handle:
            json.dump(rows, handle, indent=1)
    for key, value in sorted(counts.items()):
        print(key, value)


main()
