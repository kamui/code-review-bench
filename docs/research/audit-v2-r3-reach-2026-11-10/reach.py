"""Reach screens for the five confirmed first-grader errors of evaluator audit
round evaluator-audit-v2-2026-10-29-round-3.

Run from the repository root:

    python3 -I docs/research/audit-v2-r3-reach-2026-11-10/reach.py

It reads bench/grading/current/grades.json, the reviews it anchors to and the
round's sample and comparison, and prints one candidate list per cause. The
screens only gather candidates; the README records the judgement of each.
Nothing is written.
"""
import json
import re
from collections import defaultdict
from pathlib import Path

ROOT = Path.cwd()
AUDIT = ROOT / "bench/grading/current/audit/evaluator-audit-v2-2026-10-29-round-3"


def load(path):
    with open(path) as fh:
        return json.load(fh)


def claims():
    grades = load(ROOT / "bench/grading/current/grades.json")
    compared = {u["id"]: u for u in load(AUDIT / "comparison.json")["units"]}
    items_by_review = {}
    out = []
    for batch in grades["batches"]:
        run = batch["run"].removeprefix("runs/")
        for review in batch["reviews"]:
            for claim in review.get("claims", []):
                anchor = claim.get("anchor") or {}
                path = (anchor.get("review") or {}).get("path")
                if path not in items_by_review:
                    items_by_review[path] = load(ROOT / path).get("items", [])
                item_id = anchor.get("item_id") or ""
                index = int(item_id[5:]) if item_id[5:].isdigit() else -1
                items = items_by_review[path]
                item = items[index] if 0 <= index < len(items) else {}
                unit = compared.get(f"{run}/{review['attempt_id']}#{claim['id']}")
                out.append({
                    "run": run,
                    "target": batch["target"],
                    "attempt": review["attempt_id"],
                    "claim": claim["id"],
                    "item": item_id,
                    "quote": anchor.get("quote") or "",
                    "outcome": claim.get("outcome"),
                    "kind": claim.get("kind"),
                    "reason": claim.get("reason") or "",
                    "credit": [
                        f"{k['family_id']}:{k['says_what']}/{k['identifies_cause']}"
                        for k in claim.get("known_problems", [])
                    ],
                    "item_text": (item.get("claim") or "") + " || " + (item.get("consequence") or ""),
                    "sampled": (
                        f"sampled, second {unit['second']['result']}" if unit and unit.get("second") else ""
                    ),
                })
    return out


def show(title, rows):
    print(f"\n## {title} ({len(rows)})\n")
    for r in rows:
        tag = f" [{r['sampled']}]" if r["sampled"] else ""
        print(f"- {r['run']} {r['target']} {r['attempt']}#{r['claim']} {r['item']}: "
              f"{r['outcome']}{'/' + r['kind'] if r['kind'] else ''} {r['credit']}{tag}")
        print(f"    quote: {r['quote'][:240]}")
        print(f"    reason: {r['reason'][:320]}")


NOT_CHECKED = re.compile(
    r"\b(not|never|wasn't|weren't|isn't|without)\b[^.]{0,40}\b(check|checked|test|tested|verif|run|ran|"
    r"confirm|regenerat|exercis|validat|benchmark|measur|tried|audit|review)|\buntested\b|\bunverified\b|"
    r"\bno (test|tests|benchmark|evidence|ci)\b",
    re.I,
)
DJANGO = re.compile(
    r"ensure_timezone|timezone_name|close_pool|creator|first (thread|wrapper)|B.s wrapper|built from|whichever", re.I
)
RG_WRAPPER = re.compile(
    r"wrapper|wraps|named .?_rg|name is .?_rg|(function|helper)[^.]{0,30}_rg|"
    r"compdef[^.]{0,40}(unset|shadow|stub|later|removed|redefin|overrid|unfunction)|"
    r"(unset|shadow|stub)[^.]{0,30}compdef|wrong branch|branch (taken|is wrong)|misfire|plugin manager",
    re.I,
)
LIST_WEIGHED = re.compile(
    r"\bmost\b|one wrong item|wrong (list )?item|\bhalf\b|trivially|not the claim.s point|"
    r"the other (situation|case|half)|both (situations|cases)|neither (situation|case)",
    re.I,
)
MOVED_IN_REVIEW = re.compile(
    r"new (function|helper)s?( the change)? buil|built by (this|the) change|design of the new helpers|"
    r"even though (its|the) lines|rewritten into|new in this change|structure built",
    re.I,
)
HONO_PATTERNS = re.compile(
    r"jsonRegex|urlencodedRegex|multipartRegex|multipart (pattern|regex)|validator regex|regexes|"
    r"validator\(.(json|form).\)|/i\b",
    re.I,
)


def main():
    rows = claims()
    refuted = [r for r in rows if r["outcome"] == "refuted"]
    print(f"{len(rows)} graded claims; {len(refuted)} refuted")

    show("1. Django: stale cached timezone_name, ensure_timezone, creator wrapper (v-django-17914; "
         "quote or reason), plus any target whose reason uses the 'built from' ground",
         [r for r in rows
          if DJANGO.search(r["quote"] + " " + r["reason"])
          and (r["target"] == "v-django-17914" or re.search(r"built from B|next pool", r["reason"]))])

    show("2. Refuted claims whose quote says something was not checked, tested, verified or run",
         [r for r in refuted if NOT_CHECKED.search(r["quote"])])
    show("2b. Refuted claims whose review item (not the quote) says so",
         [r for r in refuted if NOT_CHECKED.search(r["item_text"]) and not NOT_CHECKED.search(r["quote"])])

    show("3. n-ripgrep-2957 claims about a `_rg` wrapper, compdef unset or shadowed, or a wrong branch",
         [r for r in rows if r["target"] == "n-ripgrep-2957" and RG_WRAPPER.search(r["quote"])])
    show("3b. Refuted claims, any target, whose reason weighs parts of a list",
         [r for r in refuted if LIST_WEIGHED.search(r["reason"])])
    show("3c. Refuted claims, any target, whose quote holds an 'or' list (read by hand)",
         [r for r in refuted if re.search(r"\bor\b|;", r["quote"])])

    soba = [r for r in rows if r["target"] == "q-soba-195" and r["outcome"] in ("suggestion", "problem", "minor-defect")]
    show("4. q-soba-195 claims labelled suggestion, problem or minor defect (all; read by hand)", soba)
    show("4b. Of those, reason puts a new helper or table in the review",
         [r for r in soba if MOVED_IN_REVIEW.search(r["reason"])])

    by_item = defaultdict(list)
    for r in rows:
        if r["target"] == "p-hono-5067":
            by_item[(r["run"], r["attempt"], r["item"])].append(r)
    hono = []
    for group in by_item.values():
        text = " ".join(r["quote"] for r in group) + group[0]["item_text"]
        if len(group) > 1 and HONO_PATTERNS.search(text) and len({(r["outcome"], r["kind"]) for r in group}) > 1:
            hono.extend(group)
    show("5. p-hono-5067 items with two or more claims, a validator pattern named, and different outcomes", hono)


if __name__ == "__main__":
    main()
