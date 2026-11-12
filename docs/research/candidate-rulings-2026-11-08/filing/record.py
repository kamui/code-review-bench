#!/usr/bin/env python3
"""File the owner's rulings of round candidate-rulings-2026-11-08, and question 3 of round candidate-rulings-2026-11-07,
in the current records.

    python3 -I docs/research/candidate-rulings-2026-11-08/filing/record.py

Run from the repository root. It writes the twentieth ruling receipt, then:

- GT-i4 (ruling 1, NC-d14b8d097ae3) and GT-i1 (ruling 2, NC-d75916b30d37) in the wording the owner accepted,
  `i-requests-6667/wording.approved.md`, with their impact cards, and their family and impact decisions under the new
  receipt, in the shape of the nineteenth receipt's filing of GT-i2
  (`docs/research/candidate-rulings-2026-11-07/filing/record.py`). No claim is in either family;
- for question 3 of round candidate-rulings-2026-11-07 (NC-de9bfe6de5b8 and NC-a0a4cf9ad59c, o-astro-16079), ruled
  unproven, one claim that links both comments and is unsupported, in the shape of the fifteenth receipt's filing of
  CL-v-isolation-setter-slot for its two candidates (`docs/research/rule-v4-candidates-2026-10-27/filing/record.py`);
- the decisions that close the four candidates, their `decision` fields, and the rulings in the rulings index.

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
EARLIER = ROOT / "docs/research/candidate-rulings-2026-11-07"
CURRENT = ROOT / "bench/grading/current"
CARDS = CURRENT / "impact-cards"
RECEIPT = ROOT / "bench/grading/rulings/cohort-rebuild.v20.md"
BEFORE = HERE / "before.v1.json"
INDEX = ROOT / "docs/research/cohort-rebuild-2026-10-05/second-pass/terms/rulings.index.json"
DAY = "2026-11-08"
STAMP = "2026-11-08T22:20:00Z"
TARGET = "i-requests-6667"
PROBLEM_TEXT = ("title", "obligation", "trigger", "mechanism", "grouping_reason")
LINKED = ("Linked on {day} from the comment the round lists behind {named}, as related: no blind matching was run for this "
          "filing, and the grader judges the item on its own wording.")

FAMILIES = {
    "GT-i4": {
        "n": 1, "candidate": "NC-d14b8d097ae3", "ruling": "01-requests-gt-i4-failed-cert-load",
        "widens": "The description now also covers a client certificate that fails to load while other requests run. When a "
                  "cert= request's certificate and key do not match, or its key file holds no key, the failed load leaves the "
                  "shared context presenting no certificate, and at the head other threads' good cert= requests reached their "
                  "server with no certificate, raised `requests.exceptions.SSLError`, or the process crashed. The wording is the "
                  "version the user accepted.",
        "evidence": ("rulings/01-requests-gt-i4-failed-cert-load.md", "i-requests-6667/dossiers/NC-d14b8d097ae3.md",
                     "i-requests-6667/cases/N1.md", "i-requests-6667/probes/N1/results/table.txt",
                     "i-requests-6667/probes/N1/result-head.txt", "i-requests-6667/wording.approved.md"),
        "consequence": "Run (candidate-rulings-2026-11-08 ruling 1): while another thread sends cert= with a certificate and key "
                       "that do not match, or with a key file that holds no key, the failed load leaves the shared context "
                       "presenting no certificate. At the head some of the good thread's cert= requests then reached the server "
                       "with no certificate, others raised `requests.exceptions.SSLError` although their own certificate and key "
                       "were valid, and the process died with a segmentation fault in 10 of 14 runs with a mismatched key and in "
                       "3 of 14 with a key file that holds no key. At the merge-base all 14 runs of each kind were clean. One "
                       "request at a time, a failed load leaves later requests as they are at the merge-base.",
        "exposure": "Read, for a certificate that fails to load: two public programs written before the cut-off send cert= "
                    "requests from a thread pool, each with one certificate, and neither is shown sending a certificate that "
                    "fails to load. How common it is was not measured.",
        "limits": ["Run, added on 2026-11-08 for a certificate that fails to load: a bare-context check, a one-at-a-time order "
                   "and two threads through requests at the commit before the change and at the head with Python 3.13.5, "
                   "OpenSSL 3.5.7 and urllib3 2.2.1, 14 runs per commit and kind (four of 1,000 good requests and ten of 200), "
                   "with three patched variants of the head and releases 2.32.4 and 2.32.5. Not run: urllib3 1.26.x, other "
                   "Python versions and pyOpenSSL-backed contexts.",
                   "Run on 2026-11-08: with two good certificates in two threads, the head's process crashed with a "
                   "segmentation fault in 5 of 14 runs and the merge-base's in none."],
    },
    "GT-i1": {
        "n": 2, "candidate": "NC-d75916b30d37", "ruling": "02-requests-gt-i1-proxy-manager",
        "widens": "The description now also covers an SSLContext an adapter configures through `proxy_manager_for`: for a "
                  "verify=True request sent through a proxy, the same line and the same urllib3 merge replace it with the shared "
                  "context. The wording is the version the user accepted.",
        "evidence": ("rulings/02-requests-gt-i1-proxy-manager.md", "i-requests-6667/dossiers/NC-d75916b30d37.md",
                     "i-requests-6667/cases/N2.md", "i-requests-6667/probes/N2/results/table.txt",
                     "i-requests-6667/probes/N2/result-head.txt", "i-requests-6667/wording.approved.md"),
        "consequence": "Run (candidate-rulings-2026-11-08 ruling 2): a context an adapter gives through `proxy_manager_for` is "
                       "replaced the same way for an https:// request with verify=True sent through a proxy. Through a local "
                       "http:// proxy, one that trusts a private CA gets `CERTIFICATE_VERIFY_FAILED` at the head and 200 at the "
                       "merge-base, and one that caps TLS at 1.2 connects over TLSv1.3 at the head and over TLSv1.2 at the "
                       "merge-base, with no error.",
        "exposure": "Read, for a context given through `proxy_manager_for`: its docstring exposes it for subclassing, and in 2022 "
                    "a maintainer told a user to pass a custom context through it (issue #6109). Of ten public adapters read at "
                    "their last commit before the cut-off, six set `ssl_context` in `proxy_manager_for`, each of them also in "
                    "`init_poolmanager`. How common an adapter that uses only `proxy_manager_for` is was not measured.",
        "limits": ["Run, added on 2026-11-08 for a context given through `proxy_manager_for`: adapters using "
                   "`proxy_manager_for`, `init_poolmanager` or both, through a local http:// CONNECT proxy and direct, at the "
                   "commit before the change and at the head with Python 3.13.5, OpenSSL 3.5.7 and urllib3 2.2.1, with three "
                   "patched variants of the head and releases 2.32.4 and 2.32.5. Not run: an https:// proxy, SOCKS proxies and "
                   "urllib3 1.26.x.",
                   "Run on release 2.32.4, later than the cut-off: upstream's 2.32.3 fix checks only the context "
                   "`init_poolmanager` sets, so an adapter that uses only `proxy_manager_for` still gets the shared context for "
                   "proxied requests. 2.32.5 uses the adapter's context in every case."],
    },
}

# Question 3 of round candidate-rulings-2026-11-07, answered on 2026-11-08 (answers.md entry 10).
ASTRO_RULING = "03-astro-visitor-x-astro-path"
ASTRO = {
    "target": "o-astro-16079", "candidates": ("NC-de9bfe6de5b8", "NC-a0a4cf9ad59c"), "claim": "CL-o-isr-visitor-path-first",
    "fields": {
        "trigger": "A visitor requests an ordinary ISR address of a site that uses the Vercel adapter's `isr` option with their "
                   "own query value, such as `/one?x_astro_path=/account`, and Vercel rewrites the request to "
                   "`/_isr?x_astro_path=$0`.",
        "mechanism": "The comments say that if Vercel hands the ISR function the visitor's `x_astro_path` ahead of the route's own "
                     "value, `url.searchParams.get(ASTRO_PATH_PARAM)` on `packages/integrations/vercel/src/serverless/entrypoint.ts` "
                     "line 25, which returns the first value, takes the visitor's value as the path to render.",
        "consequence": "The comments say the ISR route would then render a route the visitor chose, including one listed in "
                       "`isr.exclude`, with the platform's `x-vercel-isr` header genuinely present.",
        "change_relation": "Unproven: the change adds the branch that reads the query value, but nothing from before the cut-off "
                           "shows how Vercel combined a visitor's query with the route's destination query. Vercel's pages say "
                           "nothing on it, and its local development server puts the visitor's value in place of the route's. A "
                           "run on Vercel on 2026-11-08, later evidence only, passed only the route's value.",
        "settlement_question": "Put to the user as candidate-rulings ruling 3 on 2026-11-07, and again on 2026-11-08 after a run "
                               "on Vercel."},
    "summary": "Run in Node 24.22.2 on the entrypoint's path-override lines from each commit: at the head, a request whose query "
               "holds the visitor's `x_astro_path` first, or in place of the route's, renders the visitor's page, and one that "
               "holds the route's value first or alone renders the route; at the base every request through `/_isr` gets the "
               "404 page. Run with Bun 1.4.4: the query merge of Vercel's local development server at its last commit before "
               "the cut-off puts the visitor's value in place of the route's. Read: Vercel's pages before the cut-off say "
               "nothing on a query key present in both the request and the destination, so production Vercel's order at the "
               "cut-off is not established.",
    "later": "Known only later, never the reason for the decision (ground rule 3 of `bench/rubric/rules.pre-existing.v4.md`): on "
             "Vercel on 2026-11-08, with `@astrojs/vercel` 10.0.3 and `astro` 6.1.0, a visitor's `x_astro_path` on a rewritten "
             "ISR route did not reach the function, which saw only the route's own value. The run cannot show what Vercel did "
             "at the cut-off, 2026-03-25.",
    "reason": "Candidate-rulings ruling 3 of 2026-11-07, answered on 2026-11-08 after a run on Vercel: the user ruled it not shown "
              "(unproven), choosing the recommended option. The user's reason: \"The limit is key here. If we dont know this was "
              "the case when the PR was posted, we cna't say it's actually false can we?\" Nothing from before the cut-off "
              "shows how Vercel combined a visitor's `x_astro_path` with the route's. The run of 2026-11-08 is later evidence "
              "and, under ground rule 3 of `bench/rubric/rules.pre-existing.v4.md`, not the reason.",
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


def quoted(body):
    lines = [line[2:] for line in body.splitlines() if line.startswith("> ")]
    if len(lines) != 1:
        raise SystemExit(f"expected one quoted paragraph, found {len(lines)}")
    return lines[0]


def flat(text):
    return " ".join(text.split())


def approved(family):
    """Each field of `family` in wording.approved.md, without its bold marks, after checking that the field without its
    bold words is the current text."""
    found = sections((ROUND / "i-requests-6667/wording.approved.md").read_text(encoding="utf-8"))
    fields = {}
    for field in PROBLEM_TEXT:
        line = quoted(found[f"{family['id']} {field.replace('_', ' ')}"])
        if flat(re.sub(r"\*\*.*?\*\*", "", line)) != flat(family[field]):
            raise SystemExit(f"{family['id']} {field}: the approved text without its bold words is not the current text")
        fields[field] = line.replace("**", "")
    return fields


def demote(text, levels=2):
    return "\n".join("#" * levels + line if line.startswith("#") else line for line in text.strip().splitlines())


def named(family_id):
    return f"candidate-rulings-2026-11-08 ruling {FAMILIES[family_id]['n']}"


def recorded():
    """Every line of the receipt that records a filing, by section."""
    lines = {}
    for family_id, entry in FAMILIES.items():
        lines[family_id] = [
            f"Recorded: {entry['candidate']} of {TARGET} widens known problem {family_id}. {entry['widens']}",
            f"Recorded: the impact label of {family_id} stays serious for the widened wording.",
            f"Recorded: candidate {entry['candidate']} is closed as eligible: it is part of {family_id} ({named(family_id)}), "
            "whose wording is widened to cover it."]
    first, second = ASTRO["candidates"]
    lines["astro"] = [f"Recorded: claim {ASTRO['claim']} ({first} and {second}) is unsupported: not established.",
                      *(f"Recorded: candidate {candidate} is closed as unsupported (candidate-rulings ruling 3)."
                        for candidate in ASTRO["candidates"])]
    return lines


def scope(key, prefix):
    found = [line for line in recorded()[key] if line.startswith(prefix)]
    if len(found) != 1:
        raise SystemExit(f"{key}: no single receipt line starts {prefix!r}")
    return found[0]


def astro_answer():
    text = (EARLIER / "answers.md").read_text(encoding="utf-8")
    start = text.index("\n10. ") + 1
    return text[start:].strip()


def astro_after(candidate):
    return EARLIER / "rulings" / f"{ASTRO_RULING}.{candidate}.after.json"


def receipt_text():
    first, second = ASTRO["candidates"]
    parts = [f"""# Rulings during the issue 30 cohort rebuild, twentieth receipt

Recorded at {STAMP}. Authority: user. Issue: https://github.com/kamui/code-review-bench/issues/30.

This receipt files the user's rulings of two rounds. Round `candidate-rulings-2026-11-08` prepared the two candidates the regrade of 2026-11-08 raised on {TARGET} and asked the user one question at a time on 2026-11-08. The user closed {FAMILIES['GT-i4']['candidate']} as part of GT-i4 and {FAMILIES['GT-i1']['candidate']} as part of GT-i1, and accepted the widened wording of both; both labels stay serious. Round `candidate-rulings-2026-11-07` had left its question 3, candidates {first} and {second} on {ASTRO['target']}, pending for a test on Vercel ([nineteenth receipt](cohort-rebuild.v19.md)). After that test the user ruled them not shown (unproven) on 2026-11-08: the test shows Vercel's behaviour on 2026-11-08, not at the cut-off, and ground rule 3 of `bench/rubric/rules.pre-existing.v4.md` says later evidence is never the reason for an answer. The test is cited as later evidence only.

Each ruling section below is the file saved for its question before the user answered it: the route and check outputs, the neutral case and the first answers. Every party of these rounds was Claude, which the user allowed for round candidate-rulings-2026-11-08 in [one receipt](same-family-blind.v6.md) and for round candidate-rulings-2026-11-07 in [another](same-family-blind.v4.md). The lines after a section, starting "Recorded:", state how it is filed in the current records. After the rulings of round candidate-rulings-2026-11-08 come the user's words in that round as given and the GT-i4 and GT-i1 wording the user accepted. After question 3 come the user's words on it after the test, the entry of that round's answers written then, and the question's two after-records, since its ruling file was written before the test and the answer. Where a ruling here changes one in an earlier receipt, this receipt governs.

## Rulings of round candidate-rulings-2026-11-08
"""]
    for family_id, entry in FAMILIES.items():
        parts += [demote((ROUND / "rulings" / f"{entry['ruling']}.md").read_text(encoding="utf-8")), ""]
        parts += [line + "\n" for line in recorded()[family_id]]
    parts += ["## The user's words in round candidate-rulings-2026-11-08", "",
              demote((ROUND / "answers.md").read_text(encoding="utf-8"), 2), "",
              "## The GT-i4 and GT-i1 wording the user accepted", "",
              demote((ROUND / "i-requests-6667/wording.approved.md").read_text(encoding="utf-8"), 2), "",
              "## Question 3 of round candidate-rulings-2026-11-07", "",
              demote((EARLIER / "rulings" / f"{ASTRO_RULING}.md").read_text(encoding="utf-8"), 2), ""]
    parts += [line + "\n" for line in recorded()["astro"]]
    parts += ["## The user's words on question 3 of round candidate-rulings-2026-11-07", "",
              "From `docs/research/candidate-rulings-2026-11-07/answers.md`, entry 10, as saved:", "", astro_answer(), "",
              "## The after-records of question 3", ""]
    for candidate in ASTRO["candidates"]:
        after = astro_after(candidate)
        parts += [f"### `{after.name}`", "", "```json", after.read_text(encoding="utf-8").strip(), "```", ""]
    return "\n".join(parts).rstrip() + "\n"


def anchor(inventory, candidate_id):
    rows = [a for q in inventory["questions"] for a in q["anchors"] if a["candidate"] == candidate_id]
    if len(rows) != 1:
        raise SystemExit(f"{candidate_id}: expected one anchor")
    return rows[0]


def link(inventory, candidate_id, ruling_named):
    row = anchor(inventory, candidate_id)
    return {"review": pin(ROOT / row["review"]), "attempt_id": row["review"].split("/")[4], "item_id": row["item_id"],
            "relation": "related", "reason": f"The comment says: \"{row['quote']}\". " + LINKED.format(day=DAY, named=ruling_named)}


def main():
    if BEFORE.exists():
        raise SystemExit(f"{BEFORE.relative_to(ROOT)} is saved: this filing already ran")
    paths = {name: CURRENT / f"{name}.json" for name in ("references", "adjudications", "candidates", "claims")}
    records = {name: load(path) for name, path in paths.items()}
    index_raw = INDEX.read_text(encoding="utf-8")
    index = json.loads(index_raw)
    before = {"about": "The rows, impact cards and files of the current records that the filing of round "
                       "candidate-rulings-2026-11-08 and of question 3 of round candidate-rulings-2026-11-07 changed, as they "
                       "were before it, and the identifiers of the rows it added.",
              "families": {}, "cards": {family_id: load(CARDS / f"{family_id}.json") for family_id in FAMILIES},
              "decisions": {}, "claims": {}, "candidates": {}, "rulings_index": index_raw, "added": []}

    replace(RECEIPT, receipt_text())
    receipt = pin(RECEIPT)
    decisions = {row["id"]: row for row in records["adjudications"]["decisions"]}
    candidates = {row["id"]: row for row in records["candidates"]["candidates"]}
    claims = {row["id"]: row for row in records["claims"]["claims"]}
    targets = {target["target"]: target for target in records["references"]["targets"]}

    def add_decision(row):
        if row["id"] in decisions:
            raise SystemExit(f"{row['id']} exists")
        records["adjudications"]["decisions"].append(row)
        decisions[row["id"]] = row
        before["added"].append(row["id"])

    def close(candidate_id, target, outcome, reason, key, prefix, evidence):
        candidate = candidates[candidate_id]
        if candidate["decision"] is not None or candidate["target"] != target or candidate["revision"] != targets[target]["revision"]:
            raise SystemExit(f"{candidate_id} is not a pending candidate of {target} at its revision")
        before["candidates"][candidate_id] = copy(candidate)
        decision_id = f"AD-{candidate_id}-eligibility"
        add_decision({"id": decision_id, "target": target, "revision": candidate["revision"], "subject": candidate_id,
                      "dimension": "eligibility", "status": "approved", "outcome": outcome, "authority": "human", "reason": reason,
                      "receipt": receipt, "receipt_scope": scope(key, prefix), "boundary": None, "evidence": evidence,
                      "independent_checks": []})
        candidate["decision"] = decision_id

    # Rulings 1 and 2 of round candidate-rulings-2026-11-08: GT-i4 and GT-i1 widened.
    inventory = load(ROUND / "inventory.v1.json")
    target = targets[TARGET]
    for family_id, entry in FAMILIES.items():
        family = next(f for f in target["families"] if f["id"] == family_id)
        if any(claim["family_id"] == family_id for claim in claims.values()):
            raise SystemExit(f"a claim is in {family_id}; this filing does not re-receipt claim decisions")
        before["families"][family_id] = copy(family)
        new_evidence = [pin(ROUND / path) for path in entry["evidence"]]
        widened = f" Widened by the user on {DAY} ({named(family_id)}): {entry['widens']}"
        kept = (f" Kept by the user on {DAY} for the widened wording ({named(family_id)}): the wording the user accepted says "
                "the label stays serious. No blind session read the widened card.")
        family.update(approved(family))
        family["evidence"] += [item for item in new_evidence if item not in family["evidence"]]
        family["eligibility"]["reason"] += widened
        family["impact"]["reason"] += kept
        card = copy(before["cards"][family_id])
        card["consequence"] += " " + entry["consequence"]
        card["exposure"] += " " + entry["exposure"]
        card["limits"] += entry["limits"]
        card["grouping"]["reason"] = family["grouping_reason"]
        card["evidence"] += [item for item in new_evidence if item not in card["evidence"]]
        replace(CARDS / f"{family_id}.json", dumped(card))
        card_pin = pin(CARDS / f"{family_id}.json")
        updates = [(family["eligibility"]["adjudication"], widened, f"Recorded: {entry['candidate']} ", new_evidence),
                   (family["impact"]["adjudication"], kept, f"Recorded: the impact label of {family_id} ", [card_pin])]
        for decision_id, reason, prefix, evidence in updates:
            row = decisions[decision_id]
            if row["subject"] != family_id or row["status"] != "approved":
                raise SystemExit(f"{decision_id} is not an approved decision on {family_id}")
            before["decisions"][decision_id] = copy(row)
            old_receipt = row["receipt"]
            if row["dimension"] == "impact":
                row["evidence"] = [item for item in row["evidence"] if item["path"] != card_pin["path"]]
            row["reason"] += reason
            row["receipt"], row["receipt_scope"] = receipt, scope(family_id, prefix)
            row["evidence"] = [item for item in evidence if item not in row["evidence"]] + row["evidence"]
            if old_receipt not in row["evidence"]:
                row["evidence"].append(old_receipt)
        close(entry["candidate"], TARGET, "eligible",
              f"Closed as eligible: it is part of {family_id} ({named(family_id)}), whose wording is widened to cover it.",
              family_id, f"Recorded: candidate {entry['candidate']} ", new_evidence[:2])
        if anchor(inventory, entry["candidate"])["review"] != candidates[entry["candidate"]]["anchors"][0]["review"]["path"]:
            raise SystemExit(f"{entry['candidate']}: the round's anchor is not the candidate's")

    # Question 3 of round candidate-rulings-2026-11-07: the Astro pair, unproven.
    earlier_inventory = load(EARLIER / "inventory.v1.json")
    astro = targets[ASTRO["target"]]
    for candidate in ASTRO["candidates"]:
        if load(astro_after(candidate))["outcome"] != "unproven":
            raise SystemExit(f"{candidate}: its after-record is not unproven")
    if ASTRO["claim"] in claims:
        raise SystemExit(f"{ASTRO['claim']} exists")
    ruling = pin(EARLIER / "rulings" / f"{ASTRO_RULING}.md")
    dossiers = {candidate: pin(EARLIER / f"o-astro-16079/dossiers/{candidate}.md") for candidate in ASTRO["candidates"]}
    afters = {candidate: pin(astro_after(candidate)) for candidate in ASTRO["candidates"]}
    case = pin(EARLIER / "o-astro-16079/cases/N1.md")
    results = pin(EARLIER / "o-astro-16079/vercel-run/RESULTS.md")
    first = ASTRO["candidates"][0]
    records["claims"]["claims"].append({
        "id": ASTRO["claim"], "target": ASTRO["target"], "revision": astro["revision"], "claim": ASTRO["fields"],
        "evidence": [{"source": dossiers[first], "stance": "context", "summary": ASTRO["summary"]},
                     {"source": results, "stance": "context", "summary": ASTRO["later"]}],
        "links": [link(earlier_inventory, candidate, "candidate-rulings ruling 3") for candidate in ASTRO["candidates"]],
        "adjudication": f"AD-{ASTRO['claim']}", "family_id": None})
    before["added"].append(ASTRO["claim"])
    add_decision({"id": f"AD-{ASTRO['claim']}", "target": ASTRO["target"], "revision": astro["revision"], "subject": ASTRO["claim"],
                  "dimension": "eligibility", "status": "approved", "outcome": "unsupported", "authority": "human",
                  "reason": ASTRO["reason"], "receipt": receipt, "receipt_scope": scope("astro", f"Recorded: claim {ASTRO['claim']} "),
                  "boundary": None,
                  "evidence": [ruling, *dossiers.values(), case, *afters.values(), results], "independent_checks": []})
    for candidate in ASTRO["candidates"]:
        close(candidate, ASTRO["target"], "unsupported",
              f"Closed as unsupported: candidate-rulings ruling 3 rules its claim {ASTRO['claim']} not established.",
              "astro", f"Recorded: candidate {candidate} ", [ruling, dossiers[candidate], afters[candidate]])

    # The rulings index.
    pending = " Its question 3 is pending."
    if index["about"].count(pending) != 1:
        raise SystemExit("the rulings index does not say question 3 is pending")
    index["about"] = index["about"].replace(pending, " Its question 3 (candidate-rulings 3), answered on 2026-11-08 after a "
                                                     "test on Vercel, is named the same way.")
    index["about"] += (" Each candidate ruling of the candidate-rulings round of 2026-11-08 (candidate-rulings-2026-11-08 1 "
                       "and 2) has its own name; none showed a saved ruling again, and its file name is an alias.")
    entries = {"candidate-rulings-3": (f"{EARLIER.relative_to(ROOT).as_posix()}/rulings/{ASTRO_RULING}.md",
                                       ["candidate-rulings 3", ASTRO_RULING])}
    for family_id, entry in FAMILIES.items():
        entries[f"candidate-rulings-2026-11-08-{entry['n']}"] = (
            f"{ROUND.relative_to(ROOT).as_posix()}/rulings/{entry['ruling']}.md",
            [f"candidate-rulings-2026-11-08 {entry['n']}", entry["ruling"]])
    taken = set(index["rulings"]) | {alias for ruling in index["rulings"].values() for alias in ruling["aliases"]}
    for key, (path, aliases) in entries.items():
        if key in taken or taken & set(aliases):
            raise SystemExit(f"{key} or one of its aliases is in the rulings index")
        index["rulings"][key] = {"file": path, "aliases": aliases, "reading": "current"}

    replace(BEFORE, dumped(before))
    for name, path in paths.items():
        replace(path, dumped(records[name]))
    replace(INDEX, json.dumps(index, indent=1) + "\n")
    for line in re.findall(r"^Recorded: .*$", RECEIPT.read_text(encoding="utf-8"), re.M):
        print(line)


if __name__ == "__main__":
    main()
