#!/usr/bin/env python3
"""File the owner's rulings of round candidate-rulings-2026-11-10 in the current records.

    python3 -I docs/research/candidate-rulings-2026-11-10/filing/record.py

Run from the repository root. It writes the twenty-second ruling receipt, then:

- GT-v16 (ruling 1, the first case of NC-0266123a5aa5) in the wording the owner accepted,
  `v-django-17914/wording.approved.md`, with its impact card, and its family and impact decisions under the new receipt,
  in the shape of the twentieth receipt's filing of GT-i4 (`docs/research/candidate-rulings-2026-11-08/filing/record.py`);
- claim CL-v-pool-rebuilt-from-stale-wrapper, left unresolved by the twenty-first receipt until this grouping ruling,
  moved into GT-v16 as eligible;
- for ruling 2 (the `copy()` case of the same candidate), a new claim CL-v-pool-copy-edited-settings, advisory, in the
  shape of the sixteenth receipt's CL-v-conn-max-age-after-pool;
- the decision that closes the candidate, its `decision` field, and the two rulings in the rulings index.

`before.v1.json` saves every row, card and file it changes as it was, and the ids of the rows it adds; the script
refuses to run while that file exists."""
import hashlib
import json
import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
ROUND = HERE.parent
FOLDER = ROUND / "v-django-17914"
CURRENT = ROOT / "bench/grading/current"
CARDS = CURRENT / "impact-cards"
RECEIPT = ROOT / "bench/grading/rulings/cohort-rebuild.v22.md"
BEFORE = HERE / "before.v1.json"
INDEX = ROOT / "docs/research/cohort-rebuild-2026-10-05/second-pass/terms/rulings.index.json"
DAY = "2026-11-11"
STAMP = "2026-11-11T18:30:00Z"
TARGET = "v-django-17914"
FAMILY = "GT-v16"
CANDIDATE = "NC-0266123a5aa5"
RULINGS = {1: "01-django-gt-v16-stale-wrapper-rebuild", 2: "02-django-pool-copy-edited-settings"}
AFTERS = {1: f"{RULINGS[1]}.{CANDIDATE}.after.json", 2: f"{RULINGS[2]}.N1b.after.json"}
PROBLEM_TEXT = ("title", "obligation", "trigger", "mechanism", "grouping_reason")
CARD_ADDED = {"consequence": "Consequence", "exposure": "Exposure", "controls": "Controls"}
STALE_CLAIM = "CL-v-pool-rebuilt-from-stale-wrapper"
COPY_CLAIM = "CL-v-pool-copy-edited-settings"
COPY_ANCHOR = ("bench/runs/2026-10-02-claude-builtin-selected/attempts/att-024/normalized.json", "item-6")


def named(n):
    return f"candidate-rulings-2026-11-10 ruling {n}"


WIDENS = ("The description now also covers a pool rebuilt after an override that closes it: when another live wrapper of "
          "the alias, in another thread or made with `copy()`, still holds its old cached time zone and queries before the "
          "overriding thread, it builds the new pool, and the overriding thread's new sessions get the old time zone. The "
          "wording is the version the user accepted.")
EVIDENCE = ("rulings/01-django-gt-v16-stale-wrapper-rebuild.md", "v-django-17914/dossiers/NC-0266123a5aa5.md",
            "v-django-17914/cases/N1a.md", "v-django-17914/summary.json", "v-django-17914/probes/N1/results/table.txt",
            "v-django-17914/probes/N1/result-head.txt", "v-django-17914/wording.approved.md")
CLOSED = (f"its first case (N1a) is part of {FAMILY} ({named(1)}), whose wording is widened to cover it. Its second case "
          f"(N1b, a `copy()` with an edited time zone or role) is claim {COPY_CLAIM}, a suggestion ({named(2)}).")

STALE = {
    "change_relation": "Introduced by the change: at the commit before it there is no pool, and B's connection uses B's own "
                       f"cleared value. Part of {FAMILY} ({named(1)}), whose wording is widened to cover it.",
    "settlement_question": "Put to the user as evaluator audit round 3 reconciliation question 12 (unit #10) on 2026-11-10: "
                           "the user held that refuted was wrong. Its grouping was put to the user as "
                           f"{named(1)} on {DAY}: part of {FAMILY}.",
    "summary": "Run against PostgreSQL 16.2 with psycopg 3.1.18 and psycopg_pool 3.2.1 and 3.2.0: after the main thread B, "
               "which has used the database, overrides `TIME_ZONE` to `Africa/Nairobi`, a live thread A, or a `copy()` of B's "
               "wrapper, that queries first rebuilds the pool, and B's next session is in UTC, five of five runs. When B "
               "queries first it gets `Africa/Nairobi`; at the commit before the change it does in either order. GT-v16's saved "
               "fix leaves this order failing; a callback that reads the time zone from the settings cures it and GT-v16's new "
               "session.",
    "reason": "Evaluator audit round 3 reconciliation unit #10 of 2026-11-10: the user held the first grade wrong, agreeing "
              f"that the comment's example is not disproved. On {DAY} the user ruled it part of {FAMILY} ({named(1)}), "
              "choosing the recommended option, and accepted the widened wording.",
}

COPY = {
    "fields": {
        "trigger": "With the pool on, code makes a wrapper with `connection.copy()`, which keeps the alias, and edits the "
                   "copy's `TIME_ZONE` (with `USE_TZ = True`) or `OPTIONS[\"assume_role\"]`.",
        "mechanism": "The pool is keyed only by the alias (`django/db/backends/postgresql/base.py` head line 208), so the copy "
                     "shares the alias's pool. The pool's set-up callback belongs to whichever wrapper built it and reads "
                     "that wrapper's time zone and role (lines 231, 376 and 380 to 381).",
        "consequence": "The wrapper that reached the pool first sets up every session. The copy gets the ordinary wrapper's "
                       "time zone or the login role, or the ordinary wrapper gets the copy's time zone, with no error. A copy "
                       "with a role that builds the pool first makes every checkout time out, which is GT-v1.",
        "change_relation": "Introduced with the pool option, which the change adds: at the commit before it each wrapper sets "
                           "its own time zone and role. Outside supported use: `copy()` is undocumented, a maintainer said "
                           "before the cut-off that changing settings with the pool \"is really really really not "
                           "supported\", and the change's own tests turn the pool off for a copy they edit.",
        "settlement_question": f"Put to the user as {named(2)} on {DAY}.",
    },
    "summary": "Run at the head with PostgreSQL 16.2, psycopg 3.1.18 and psycopg_pool 3.2.1: a copy with `TIME_ZONE = "
               "\"Asia/Tokyo\"` that queries first gives the ordinary wrapper `Asia/Tokyo`; when the ordinary wrapper is "
               "first, the copy gets UTC; a copy with `assume_role` gets the login role when the ordinary wrapper is first. "
               "At the base each wrapper gets its own settings. A second alias in `DATABASES` gets its own pool and its own "
               "time zone at the head.",
    "reason": f"{named(2)} of {DAY}: the user ruled it a suggestion, outside supported use, choosing the recommended "
              "option, as in rule-v4-post-filing ruling 2 (`CONN_MAX_AGE` edited after the pool exists). A copy of a wrapper "
              "with an edited time zone or role shares the alias's pool, so whichever wrapper builds the pool sets up the "
              "other's sessions. `copy()` is undocumented, and before the cut-off a maintainer said changing settings with "
              "the pool \"is really really really not supported\". It stays off the answer key: a suggestion or "
              "observation, of the type outside supported use.",
    "evidence": ("rulings/02-django-pool-copy-edited-settings.md", "v-django-17914/dossiers/N1b.md",
                 "v-django-17914/cases/N1b.md", "v-django-17914/probes/N1/results/table.txt"),
}


def load(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def dumped(value):
    return json.dumps(value, indent=2) + "\n"


def replace(path, text):
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(text, encoding="utf-8")
    os.replace(temporary, path)


def pin(path):
    path = Path(path).resolve()
    return {"path": path.relative_to(ROOT.resolve()).as_posix(), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def copy(value):
    return json.loads(json.dumps(value))


def sections(text):
    found, name = {}, None
    for line in text.splitlines():
        if line.startswith("## "):
            name = line[3:].strip()
            found[name] = []
        elif name:
            found[name].append(line)
    return {name: "\n".join(lines).strip() for name, lines in found.items()}


def flat(text):
    return " ".join(text.split())


def wording():
    return sections((FOLDER / "wording.approved.md").read_text(encoding="utf-8"))


def approved(family):
    """Each field of the family in wording.approved.md, without its bold marks, after checking that the field without its
    bold words is the current text."""
    found = wording()
    fields = {}
    for field in PROBLEM_TEXT:
        lines = [line[2:] for line in found[f"{FAMILY} {field.replace('_', ' ')}"].splitlines() if line.startswith("> ")]
        if len(lines) != 1:
            raise SystemExit(f"{field}: expected one quoted paragraph")
        line = lines[0]
        if flat(re.sub(r"\*\*.*?\*\*", "", line)) != flat(family[field]):
            raise SystemExit(f"{FAMILY} {field}: the approved text without its bold words is not the current text")
        fields[field] = line.replace("**", "")
    return fields


def card_additions():
    """The quoted sentences the approved wording adds to each card field, and to its limits."""
    body = wording()[f"{FAMILY} impact card"]
    added = {}
    for key, label in {**CARD_ADDED, "limits": "Limits"}.items():
        match = re.search(rf'^- {label}, added: "(.*)"$', body, re.M)
        if not match:
            raise SystemExit(f"impact card: no added {label}")
        added[key] = match.group(1)
    return added


def demote(text, levels=2):
    return "\n".join("#" * levels + line if line.startswith("#") else line for line in text.strip().splitlines())


def recorded():
    """Every line of the receipt that records a filing, by section."""
    return {
        1: [f"Recorded: {CANDIDATE} of {TARGET} widens known problem {FAMILY}. {WIDENS}",
            f"Recorded: the impact label of {FAMILY} stays other-material for the widened wording.",
            f"Recorded: claim {STALE_CLAIM} (audit round 3 unit #10) is eligible, in known problem {FAMILY} ({named(1)}).",
            f"Recorded: candidate {CANDIDATE} is closed as eligible: {CLOSED}"],
        2: [f"Recorded: claim {COPY_CLAIM} (N1b) is advisory: a suggestion or observation, of the type outside supported use."],
    }


def scope(n, prefix):
    found = [line for line in recorded()[n] if line.startswith(prefix)]
    if len(found) != 1:
        raise SystemExit(f"ruling {n}: no single receipt line starts {prefix!r}")
    return found[0]


def receipt_text():
    parts = [f"""# Rulings during the issue 30 cohort rebuild, twenty-second receipt

Recorded at {STAMP}. Authority: user. Issue: https://github.com/kamui/code-review-bench/issues/30.

This receipt files the user's rulings of round `candidate-rulings-2026-11-10`. The round prepared candidate {CANDIDATE}, which the regrade under the [twenty-first receipt](cohort-rebuild.v21.md) raised on {TARGET}, as two questions, and asked the user both in one chat message on 2026-11-11. The user ruled its first case, a pool rebuilt by another live wrapper after a time zone override, part of {FAMILY} with the wording widened, and accepted the widened wording; the label stays other-material. The user ruled its second case, a `copy()` of a wrapper with an edited time zone or role, a suggestion, outside supported use. Claim {STALE_CLAIM}, which the twenty-first receipt left unresolved until this grouping ruling, moves into {FAMILY}.

Each ruling section below is the file saved for its question before the user answered it: the route and check outputs, the neutral case and the first answers. Every party of this round was Claude, which the user allowed in [one receipt](same-family-blind.v8.md). The lines after a section, starting "Recorded:", state how it is filed in the current records. After the rulings come the user's words in the round as given and the {FAMILY} wording the user accepted. Where a ruling here changes one in an earlier receipt, this receipt governs.

## Rulings of round candidate-rulings-2026-11-10
"""]
    for n, ruling in RULINGS.items():
        parts += [demote((ROUND / "rulings" / f"{ruling}.md").read_text(encoding="utf-8")), ""]
        parts += [line + "\n" for line in recorded()[n]]
    parts += ["## The user's words in round candidate-rulings-2026-11-10", "",
              demote((ROUND / "answers.md").read_text(encoding="utf-8"), 2), "",
              f"## The {FAMILY} wording the user accepted", "",
              demote((FOLDER / "wording.approved.md").read_text(encoding="utf-8"), 2), ""]
    return "\n".join(parts).rstrip() + "\n"


def main():
    if BEFORE.exists():
        raise SystemExit(f"{BEFORE.relative_to(ROOT)} is saved: this filing already ran")
    for n, after in AFTERS.items():
        outcome = load(ROUND / "rulings" / after)["outcome"]
        if outcome != {1: "duplicate", 2: "suggestion"}[n]:
            raise SystemExit(f"{after}: outcome {outcome}")
    paths = {name: CURRENT / f"{name}.json" for name in ("references", "adjudications", "candidates", "claims")}
    records = {name: load(path) for name, path in paths.items()}
    index_raw = INDEX.read_text(encoding="utf-8")
    index = json.loads(index_raw)
    before = {"about": "The rows, impact card and file of the current records that the filing of round "
                       "candidate-rulings-2026-11-10 changed, as they were before it, and the identifiers of the rows it added.",
              "families": {}, "cards": {FAMILY: load(CARDS / f"{FAMILY}.json")}, "decisions": {}, "claims": {},
              "candidates": {}, "rulings_index": index_raw, "added": []}

    replace(RECEIPT, receipt_text())
    receipt = pin(RECEIPT)
    decisions = {row["id"]: row for row in records["adjudications"]["decisions"]}
    candidates = {row["id"]: row for row in records["candidates"]["candidates"]}
    claims = {row["id"]: row for row in records["claims"]["claims"]}
    target = next(t for t in records["references"]["targets"] if t["target"] == TARGET)

    def add_decision(row):
        if row["id"] in decisions:
            raise SystemExit(f"{row['id']} exists")
        records["adjudications"]["decisions"].append(row)
        decisions[row["id"]] = row
        before["added"].append(row["id"])

    def rereceipt(row, reason, prefix, n, evidence, append=True):
        before["decisions"][row["id"]] = copy(row)
        old_receipt = row["receipt"]
        row["reason"] = row["reason"] + reason if append else reason
        row["receipt"], row["receipt_scope"] = receipt, scope(n, prefix)
        row["evidence"] = [item for item in evidence if item not in row["evidence"]] + row["evidence"]
        if old_receipt not in row["evidence"]:
            row["evidence"].append(old_receipt)

    # Ruling 1: GT-v16 widened.
    family = next(f for f in target["families"] if f["id"] == FAMILY)
    before["families"][FAMILY] = copy(family)
    new_evidence = [pin(ROUND / path) for path in EVIDENCE]
    widened = f" Widened by the user on {DAY} ({named(1)}): {WIDENS}"
    kept = (f" Kept by the user on {DAY} for the widened wording ({named(1)}): the wording the user accepted says the "
            "impact band stays other-material. No blind session read the widened card.")
    family.update(approved(family))
    family["evidence"] += [item for item in new_evidence if item not in family["evidence"]]
    family["eligibility"]["reason"] += widened
    family["impact"]["reason"] += kept
    card = copy(before["cards"][FAMILY])
    added = card_additions()
    for key in CARD_ADDED:
        card[key] += " " + added[key]
    card["limits"].append(added["limits"])
    card["grouping"]["reason"] = family["grouping_reason"]
    card["evidence"] += [item for item in new_evidence if item not in card["evidence"]]
    replace(CARDS / f"{FAMILY}.json", dumped(card))
    card_pin = pin(CARDS / f"{FAMILY}.json")
    for decision_id, reason, prefix, evidence in [
            (family["eligibility"]["adjudication"], widened, f"Recorded: {CANDIDATE} ", new_evidence),
            (family["impact"]["adjudication"], kept, f"Recorded: the impact label of {FAMILY} ", [card_pin])]:
        row = decisions[decision_id]
        if row["subject"] != FAMILY or row["status"] != "approved":
            raise SystemExit(f"{decision_id} is not an approved decision on {FAMILY}")
        if row["dimension"] == "impact":
            row["evidence"] = [item for item in row["evidence"] if item["path"] != card_pin["path"]]
        rereceipt(row, reason, prefix, 1, evidence)

    # Ruling 1: the stale-wrapper claim moves into GT-v16.
    stale = claims[STALE_CLAIM]
    row = decisions[stale["adjudication"]]
    if stale["family_id"] is not None or row["outcome"] != "unresolved":
        raise SystemExit(f"{STALE_CLAIM} is not the unresolved claim the twenty-first receipt filed")
    before["claims"][STALE_CLAIM] = copy(stale)
    stale["claim"]["change_relation"] = STALE["change_relation"]
    stale["claim"]["settlement_question"] = STALE["settlement_question"]
    stale["evidence"].append({"source": new_evidence[1], "stance": "supports", "summary": STALE["summary"]})
    stale["family_id"] = FAMILY
    rereceipt(row, STALE["reason"], f"Recorded: claim {STALE_CLAIM} ", 1,
              [new_evidence[0], new_evidence[1], new_evidence[2], pin(ROUND / "rulings" / AFTERS[1])], append=False)
    row["outcome"] = "eligible"

    # Ruling 2: the copy() case, a suggestion.
    if COPY_CLAIM in claims:
        raise SystemExit(f"{COPY_CLAIM} exists")
    candidate = candidates[CANDIDATE]
    anchor = next(a for a in candidate["anchors"] if (a["review"]["path"], a["item_id"]) == COPY_ANCHOR)
    copy_evidence = [pin(ROUND / path) for path in COPY["evidence"]]
    records["claims"]["claims"].append({
        "id": COPY_CLAIM, "target": TARGET, "revision": target["revision"], "claim": COPY["fields"],
        "evidence": [{"source": copy_evidence[1], "stance": "context", "summary": COPY["summary"]}],
        "links": [{"review": anchor["review"], "attempt_id": COPY_ANCHOR[0].split("/")[4], "item_id": anchor["item_id"],
                   "relation": "related",
                   "reason": f"The comment says: \"{anchor['quote']}\". Linked on {DAY} from the comment the round lists "
                             f"behind {named(2)}, as related: no blind matching was run for this filing, and the grader "
                             "judges the item on its own wording."}],
        "adjudication": f"AD-{COPY_CLAIM}", "family_id": None})
    before["added"].append(COPY_CLAIM)
    add_decision({"id": f"AD-{COPY_CLAIM}", "target": TARGET, "revision": target["revision"], "subject": COPY_CLAIM,
                  "dimension": "eligibility", "status": "approved", "outcome": "advisory", "authority": "human",
                  "reason": COPY["reason"], "receipt": receipt, "receipt_scope": scope(2, f"Recorded: claim {COPY_CLAIM} "),
                  "boundary": None, "evidence": [*copy_evidence, pin(ROUND / "rulings" / AFTERS[2])],
                  "independent_checks": []})

    # The candidate.
    if candidate["decision"] is not None or candidate["target"] != TARGET or candidate["revision"] != target["revision"]:
        raise SystemExit(f"{CANDIDATE} is not a pending candidate of {TARGET} at its revision")
    before["candidates"][CANDIDATE] = copy(candidate)
    decision_id = f"AD-{CANDIDATE}-eligibility"
    add_decision({"id": decision_id, "target": TARGET, "revision": candidate["revision"], "subject": CANDIDATE,
                  "dimension": "eligibility", "status": "approved", "outcome": "eligible", "authority": "human",
                  "reason": f"Closed as eligible: {CLOSED}", "receipt": receipt,
                  "receipt_scope": scope(1, f"Recorded: candidate {CANDIDATE} "), "boundary": None,
                  "evidence": [new_evidence[0], new_evidence[1], copy_evidence[0],
                               *(pin(ROUND / "rulings" / after) for after in AFTERS.values())],
                  "independent_checks": []})
    candidate["decision"] = decision_id

    # The rulings index.
    index["about"] += (" Each candidate ruling of the candidate-rulings round of 2026-11-10 (candidate-rulings-2026-11-10 1 "
                       "and 2) has its own name; none showed a saved ruling again, and its file name is an alias.")
    taken = set(index["rulings"]) | {alias for ruling in index["rulings"].values() for alias in ruling["aliases"]}
    for n, ruling in RULINGS.items():
        key = f"candidate-rulings-2026-11-10-{n}"
        aliases = [f"candidate-rulings-2026-11-10 {n}", ruling]
        if key in taken or taken & set(aliases):
            raise SystemExit(f"{key} or one of its aliases is in the rulings index")
        index["rulings"][key] = {"file": f"{ROUND.relative_to(ROOT).as_posix()}/rulings/{ruling}.md", "aliases": aliases,
                                 "reading": "current"}

    replace(BEFORE, dumped(before))
    for name, path in paths.items():
        replace(path, dumped(records[name]))
    replace(INDEX, json.dumps(index, indent=1) + "\n")
    for line in re.findall(r"^Recorded: .*$", RECEIPT.read_text(encoding="utf-8"), re.M):
        print(line)


if __name__ == "__main__":
    main()
