# Filing the v3 regrade rulings in the current records

[`record.py`](record.py) files the rulings of this round from [`plan.v1.json`](plan.v1.json), the five pending candidates in `bench/grading/current/candidates.json` and the comments listed in [`../inventory.v1.json`](../inventory.v1.json), against the [twelfth receipt](../../../../bench/grading/rulings/cohort-rebuild.v12.md). It follows the v10 round's [`record.py`](../../regrade-v10-rulings-2026-10-17/filing/record.py), and the last-push round's for the new known problem, so its files change together or not at all.

The owner answered the round's ten rulings on 2026-10-23. Each ruling file under [`../rulings/`](../rulings/) ends with the answer, [`../owner-answers.md`](../owner-answers.md) holds the owner's words, and those are the authority for everything here. The receipt holds both.

| What | Before | After |
| --- | ---: | ---: |
| Known problems | 71 | 72 |
| of them serious | 31 | 32 |
| Pre-existing problems | 14 | 16 |
| of them serious | 8 | 10 |
| Decisions | 461 | 482 |
| Claims | 126 | 136 |
| Links from claims to comments | 1,180 | 1,196 |
| of them equivalent | 456 | 452 |
| Candidates | 167 | 167 |
| of them open | 5 | 0 |
| Rulings on single comments | 34 | 38 |

| What the rulings did | Count |
| --- | ---: |
| New pre-existing problems | 2 |
| New known problems | 1 |
| Known problems with widened wording | 1 |
| Pre-existing problems with widened wording | 2 |
| Claims kept off the answer key | 6 |
| of them saved claims confirmed | 2 |
| of them outside this change | 2 |
| of them suggestions | 3 |
| of them refuted | 1 |
| Rulings on one comment's credit or raising | 4 |
| of them credit or raised | 1 |

## The wording is a draft for the owner

The decisions are the owner's. The text of the two new pre-existing problems, the new known problem and the three widened entries is the recording session's, drafted from each ruling's dossier, case, card and runs. So are the concern tags of the two new pre-existing problems and the type of the two new suggestions. [`entries-for-approval.md`](entries-for-approval.md) shows each, and `record.py` writes that file from the plan each time it files. The owner has not accepted it yet.

`record.py --undo` reads [`before.v1.json`](before.v1.json), which holds every row and card the filing changed as it was and the files it created, and puts them back byte for byte. An undo followed by a new filing was run and gave the same bytes. If the owner changes the wording, undo, change the plan, file again.

## Pre-existing problems

- **Two are new**, each admitted by the owner: `PE-q5` (ruling 1, part of a BitBucket credential set) on `q-soba-195`, and `PE-s6` (ruling 4, an expired directory's list of children left by a listing) on `s-seaweedfs-10735`. Each has an admission decision, an impact card, an impact decision with the blind inspector's check and a claim filed as outside this change. `q-soba-195` stays an audited-clean control: a pre-existing problem is not a known problem.
- **Two are widened**: `PE-q1` by ruling 3 (GitHub's skip-user-repos switch given through `_FILE`) and `PE-j1` by ruling 6 (a function input after a middleware). Each keeps its label. Their admission and impact decisions cite this receipt and keep the pin of the one they cited before. Their saved claims keep their wording, and their decisions cite the line that says each stays in it.
- **Concern tags are the recording session's**: Reliability for both, as for `PE-q4` and `PE-s2`.

## Known problems

- **`GT-v14` is new** (ruling 9): with the pool on, a `keepdb` teardown leaves the alias's pool on the test database. It has an eligibility decision, an impact card and an impact decision with the blind inspector's check, and a new eligible claim. `bench/profiles.json` has no short label or concern tags for it yet; the explorer export needs them before it runs.
- **`GT-y2` is widened** (ruling 10) to a parallel request whose session save is answered with HTTP 400. It keeps its label. Its title is replaced, its fields and impact card gain the added symptom with its runs, and its eligibility and impact decisions cite this receipt and keep the pin of the one they cited before.

## Labels

One blind label inspector, Codex GPT-6.1 Sol, and the asking session's own first label. Only the inspector is recorded as an independent check.

| Entry | The owner's label | The inspector | The asking session |
| --- | --- | --- | --- |
| `PE-q5` | serious | serious, high | serious |
| `PE-s6` | serious | serious, high | serious |
| `GT-v14` | serious | serious, medium | serious |

For the three widened entries no inspector read the widened card; the inspector labelled each added symptom by itself, in case the owner kept it apart, and that answer is in the impact decision's reason.

## Off the answer key

| Ruling | Claim | Filed as |
| --- | --- | --- |
| 2 | `CL-q-backup-dir-trailing-newline` | scope-excluded: outside this change, not admitted |
| 7 | `CL-j-date-map-set-input` | the saved claim, confirmed: scope-excluded (regrade-v10 ruling 9) |
| 8 | `CL-r-flip-render` | the saved claim, confirmed: advisory, a suggestion (regrade-v10 ruling 18) |
| 8 | `CL-r-render-function-stale-marks`, `CL-r-bundle-size-increase` | advisory: a suggestion or observation, of the type improvement |
| 8 | `CL-r-benchmark-timings` | refuted |

The two confirmed claims gain a link to the comment the round asked about, and their decisions cite this receipt as well. The ruling index already names every ruling of the round, so this filing does not change it.

Version 3 of the rule text calls a suggestion's kind its type, so the new records say "of the type". The saved records say "of the kind", as the text in force when they were filed did.

## Candidates

The five candidates were pending in `candidates.json`, and each is closed with a decision: two admitted as `PE-q5` and `PE-s6`, one not admitted (ruling 2, a `pre-existing-admission` decision with the outcome `not-admitted`), one eligible as the new known problem `GT-v14`, and one eligible through the widened `GT-y2`.

## Credit and raising

Four rows in `credits.json`, with both facts. Three are for pre-existing problems, where a "yes" on "says what goes wrong" means the comment raises it.

| Ruling | Entry | Comment | Says what goes wrong | Identifies the cause as a fault |
| --- | --- | --- | --- | --- |
| 3 | `PE-q1` | C1 | yes | yes |
| 5 | `PE-s4` | C1 | no | cannot tell |
| 6 | `PE-j1` | C1 | no | yes |
| 8 | `GT-r10` | C1 | no | yes |

The credit schema has no "cannot tell" for the second fact, so ruling 5's row stores `null`, the value for no ruling on it, and its reason and receipt passage give the owner's answer.

## Links

Each of the ten new claims is linked to the comments the round lists behind its ruling, and each confirmed claim to one more: 16 links, each `related`, with the comment's own words in the reason. Where a ruling filed one part of a comment, the reason quotes that part. No blind matching was run.

Four saved links were narrowed from equivalent to related, by the v10 filing's rule that a comment this filing links to a claim is linked to no other claim as equivalent:

- `CL-v-test-pool-database` (`GT-v2`): the three comments behind ruling 9, which now also link to `CL-v-keepdb-teardown-pool`.
- `CL-y-fallback-cycle-key` (`GT-y2`): the comment behind ruling 10, which now also links to `CL-y-parallel-session-write-lost`.

The grader now judges these four comments on their own wording for those claims, where before they carried the claims' saved outcomes.

## What it makes stale

`bench-tools current-grading check` now stops at the first stale grade, and `bun run verify:current` with it. `bench-tools current-grading fingerprints` lists 60 of the 198 batches as stale:

| Task | Stale batches | Why |
| --- | ---: | --- |
| `j-trpc-5017` | 13 of 13 | `PE-j1` widened |
| `q-soba-195` | 14 of 14 | `PE-q5` added, `PE-q1` widened |
| `s-seaweedfs-10735` | 14 of 14 | `PE-s6` added |
| `v-django-17914` | 9 of 9 | `GT-v14` added |
| `y-django-16631` | 9 of 9 | `GT-y2` widened |
| `r-base-ui-5460` | 1 of 14 | `runs/2026-10-08-last-push-claude-sonnet`, whose comment gains claim links and a credit ruling |

## How it differs from the v10 filing

- **The Python modules are gone.** The v10 script imports `current_grading.py`, `claims.py` and `calibration.py`, which the move to bench-tools deleted. `record.py` copies the few helpers it took from them, and from the last-push round's script, and validates through bench-tools instead: `current-grading fingerprints`, which validates the records as `check` does but leaves saved grades unchecked, `calibration queue` and `calibration check`. A failure puts every file back.
- **No candidate records to add.** The regrade saved its candidates in `candidates.json`, so no `candidates.v1.json` is needed.

## What this filing does not do

- **It grades nothing.** The 60 stale batches keep their grades until a regrade, which needs a plan and the owner's authorization. No batch was invalidated.
- **It ran no blind session and no review.** No matcher linked the comments, and no inspector read the widened cards.
- **It does not add `GT-v14` to `bench/profiles.json`.**
