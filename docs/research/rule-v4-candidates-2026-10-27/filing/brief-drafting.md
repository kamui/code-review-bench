# Brief: draft the filing entries of the rule-v4 candidate round

You draft answer-key wording for a code-review benchmark. Work in the repository at `/home/jack/.cache/regrade-v4` (a git worktree, branch `t3code/issue-374-regrade-v4`). Write only your one output file under `docs/research/rule-v4-candidates-2026-10-27/filing/drafts/`. Do not edit any other file, do not commit, do not run reviews, graders or anything paid, and do not delete anything.

## What happened

The round `docs/research/rule-v4-candidates-2026-10-27/` asked the owner 29 questions about claims that review comments raised and that matched no known problem. The round is closed. Read its `README.md` first (decisions table), then `owner-answers.md` (the owner's words, the authority) and `reconcile.v1.md` (why recommendations changed). Each ruling file is `rulings/NN-*.md`; it holds the case text and the first answers. The preparation evidence is under each target folder (`<target>/dossiers/`, `cases/`, `cards/`, `probes/`, `upstream/`, `REPORT.md`). Facts come only from those files and the repository. Do not run new experiments; reading code under the bench mirrors is fine if you need a line number.

Your job: turn each decision into the filing entries that `record.py` will write into `bench/grading/current/` (references, impact cards, claims, adjudications, candidates). The owner will read the result as `entries-for-approval.md`, so the wording is what the owner approves.

## Format: copy the earlier filings exactly

The schema is the v3 filing plan, `docs/research/regrade-v3-rulings-2026-10-23/filing/plan.v1.json` (keys `new_pre_existing`, `new_families`, `widened`, `off_key`), and its rendering `.../filing/entries-for-approval.md`. The v10 filing plan, `docs/research/regrade-v10-rulings-2026-10-17/filing/plan.v1.json`, has more `off_key` and `widened` examples. Match every field name and kind of content in those entries, including `card` (with `domain`, `attribution`, `consequence`, `exposure`, `controls`, `reversibility`, `workload`, `change_activity`, `grouping`, `limits`), `claim_text` and `summary`. Look at existing rows in `bench/grading/current/references.json` and cards in `bench/grading/current/impact-cards/` for the same target to match tone and detail. For a pre-existing problem also give `concerns` (concern tags from the vocabulary used in `bench/profiles.json`). For a new known problem also give a `profile` object: `{"label": "<short site label, about 40 characters>", "concerns": [...]}` like the `labels` and `findings` in `bench/profiles.json`.

Use these fields for each entry: `ruling` and `question` (the ruling number), `file` (the ruling file name), `target`, `group` (the ruling's group, e.g. `M4a`), `inventory_group` (the group in `inventory.v1.json`, e.g. `M4`), `candidate` (the NC ids that this decision closes), `claim` (a new claim id `CL-<target letter>-<short-slug>`, unique; check `bench/grading/current/claims.json` for collisions), and the label (`band` for a known problem, `label` for a pre-existing problem) as the owner ruled. The `ruled` sentence says what the user ruled, citing "rule-v4-candidates ruling N of 2026-10-27" or "of 2026-10-28" (the date is in `owner-answers.md`).

`off_key` outcomes: unproven is `unsupported`; suggestion is `advisory` with a `type` ("improvement" or "outside supported use"); minor defect is `inconsequential`; outside this change is `scope-excluded`; a not-admitted pre-existing case is `scope-excluded` with `not_admitted: true`. Each `off_key` entry has a `claim_text` and `summary` like the examples.

## Splits

Three inventory groups were split into two rulings: `v-django-17914` M4 (rulings 05 M4a and 06 M4b, one candidate), `p-hono-5067` M4 (rulings 13 M4a and 14 M4b, two candidates; the owner ruled them one problem), and `s-seaweedfs-10735` M1 (rulings 25 M1a and 26 M1b, five candidates). For a split you handle, read each anchor's quote and comment in `inventory.v1.json` and assign each candidate and each comment to the ruling it raises. Give the assignment and the reason in a top-level `notes` list in your output. If one candidate raises both halves, list it under both and say so.

## Writing

The owner finds the current text jargon-heavy and hard to parse. Write plain, literal sentences, one idea each, with articles and verbs. Name real files, functions and line numbers. Mark facts *Run*, *Read* or *Reported* as the examples do. No em dashes, no semicolons strung into long sentences where two sentences read better, no adverbs propping up verbs, no metaphors, no "crucial", "robust", "seamless" and the like. Do not invent numbers; every number must appear in the round's files. Where a fact is missing, say it is not established, as the examples do. Keep the owner's decisions exactly: a label, a grouping or an outcome is never yours to change. If you think a decision cannot be written as ruled, say so in `notes` and draft it as ruled anyway.

## Output

One JSON file, `docs/research/rule-v4-candidates-2026-10-27/filing/drafts/<your-cluster>.json`, with the keys `new_families`, `new_pre_existing`, `widened`, `off_key` (each a list, empty if none) and `notes` (a list of strings). Validate that it parses with `python3 -c 'import json,sys; json.load(open(sys.argv[1]))' <file>`. Then reply with a short summary: the entries written, the split assignments, and anything uncertain.
