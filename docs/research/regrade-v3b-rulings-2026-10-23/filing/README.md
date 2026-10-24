# Filing the v3b regrade rulings in the current records

[`record.py`](record.py) files the two rulings of this round from [`plan.v1.json`](plan.v1.json), the pending candidate in `bench/grading/current/candidates.json` and the comments listed in [`../inventory.v1.json`](../inventory.v1.json), against the [thirteenth receipt](../../../../bench/grading/rulings/cohort-rebuild.v13.md). It follows the v3 round's [`record.py`](../../regrade-v3-rulings-2026-10-23/filing/record.py), keeping only the parts this round uses, so its files change together or not at all.

The owner answered both rulings on 2026-10-23. Each ruling file under [`../rulings/`](../rulings/) ends with the answer, [`../owner-answers.md`](../owner-answers.md) holds the owner's words, and those are the authority for everything here. The receipt holds both.

| What | Before | After |
| --- | ---: | ---: |
| Known problems | 72 | 72 |
| of them serious | 32 | 32 |
| Pre-existing problems | 16 | 16 |
| of them serious | 10 | 10 |
| Decisions | 482 | 484 |
| Claims | 136 | 137 |
| Links from claims to comments | 1,196 | 1,198 |
| of them equivalent | 452 | 452 |
| Candidates | 168 | 168 |
| of them open | 1 | 0 |
| Rulings on single comments | 38 | 39 |

| What the rulings did | Count |
| --- | ---: |
| New pre-existing problems | 0 |
| New known problems | 0 |
| Entries with widened wording | 0 |
| Claims kept off the answer key | 2 |
| of them saved claims confirmed | 1 |
| of them outside this change | 1 |
| of them suggestions | 1 |
| Rulings on one comment's credit | 1 |
| of them credit | 0 |

## The claim text is the owner's approved text

Filing ruling 01 as the v3 and v10 filings filed a not-admitted candidate needs a new claim, whose wording the owner had not seen. The filing stopped before writing anything and asked. The owner approved the claim text on 2026-10-23 ([`answers.md`](answers.md)), and the receipt holds that answer. `CL-q-startup-summary-bitbucket-gate` carries that text word for word as its trigger, mechanism, consequence, relation to the change and ruled sentence. Only the relation to the change says "Outside this change" where the shown bullet said "Outside it", because the field has no label for "it" to refer to. Its evidence summary restates the runs of the round's case file.

`record.py --undo` reads [`before.v1.json`](before.v1.json), which holds every row the filing changed as it was and the file it created, and puts them back byte for byte. An undo followed by a new filing was run and gave the same bytes.

## Off the answer key

| Ruling | Claim | Filed as |
| --- | --- | --- |
| [01](../rulings/01-soba-startup-summary.md) | `CL-q-startup-summary-bitbucket-gate` | scope-excluded: outside this change, not admitted |
| [02](../rulings/02-base-ui-disabled-baseline.md) | `CL-r-late-load-dirty` | the saved claim, confirmed: advisory, a suggestion of the type improvement (second-pass ruling 17) |

The new claim is linked to the two comments behind the candidate, each `related`, with the comment's own words in the reason. No blind matching was run. The comment behind ruling 02 was already linked to `CL-r-late-load-dirty` as related, so that claim's links are unchanged; its decision cites this receipt and keeps the pin of the seventh receipt it cited before. No saved link was narrowed.

The rulings index gains `regrade-v3b-1` and `regrade-v3b-2`, each with the alias `regrade-v3b N` and its file name, which the round's records use.

## Candidates

`NC-9e4dd0082a4c` was pending in `candidates.json`. It is closed with a `pre-existing-admission` decision with the outcome `not-admitted`. `q-soba-195` is an audited-clean control again in the decision queue.

## Credit

One row in `credits.json`, with both facts.

| Ruling | Known problem | Comment | Says what goes wrong | Identifies the cause as a fault |
| --- | --- | --- | --- | --- |
| 02 | `GT-r11` | C1 | no | yes |

## What it makes stale

`bench-tools current-grading check` now stops at the first stale grade, and `bun run verify:current` with it. `bench-tools current-grading fingerprints` lists 5 of the 198 batches as stale:

| Task | Run | Why |
| --- | --- | --- |
| `q-soba-195` | `runs/2026-10-08-last-push-claude-opus` | its two comments are linked to the new claim |
| `r-base-ui-5460` | `runs/2026-10-08-last-push-claude-sonnet` | its comment gains a credit ruling, and the decision of `CL-r-late-load-dirty`, which it is linked to, cites this receipt |
| `r-base-ui-5460` | `runs/2026-10-08-last-push-claude-fable` | linked to `CL-r-late-load-dirty` |
| `r-base-ui-5460` | `runs/2026-10-08-last-push-claude-opus` | linked to `CL-r-late-load-dirty` |
| `r-base-ui-5460` | `runs/2026-10-10-last-push-claude-thermo-opus` | linked to `CL-r-late-load-dirty` |

## What this filing does not do

- **It grades nothing.** The 5 stale batches keep their grades until a regrade. No batch was invalidated.
- **It ran no blind session and no review.** No matcher linked the comments.
