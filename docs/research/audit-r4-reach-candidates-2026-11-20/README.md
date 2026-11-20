# Candidate round audit-r4-reach-candidates-2026-11-20

Part of issue #30. Evaluator audit round 4 unit #14 found a cause graded apart from the result that refers back to it, and the owner held the two one thought. The reach check of that error found four more comments graded the same way. A regrade alone repeats the split, so each needed the owner's call before it could be filed. Under answer 3 ("b") of 2026-11-20 to the corrections questions (see [`../audit-v2-r4-corrections-2026-11-20/README.md`](../audit-v2-r4-corrections-2026-11-20/README.md)), this round asked those four.

| Question | Comment | Case | Owner's answer |
| --- | --- | --- | --- |
| 1 | sonnet `p-hono-5067` `att-007` item-2: "That is a much broader change than the matching guard needed." | [`K1`](p-hono-5067/cases/K1.md) | duplicate of GT-p3 |
| 2 | sonnet `p-hono-5067` `att-029` item-1: "The fix is at the wrong depth." | [`K2`](p-hono-5067/cases/K2.md) | duplicate of GT-p3 |
| 3 | builtin-selected `v-django-17914` `att-015` item-4: "`_configure_connection` runs from pool worker threads but reads `self.ops`, `self.timezone_name` and `self.settings_dict`. …" | [`K3`](v-django-17914/cases/K3.md) | duplicate of GT-v16 |
| 4 | fable `n-ripgrep-2957` `att-005` item-4: the no-`compdef` fallback "exists only so `ci/test-complete` can keep dumping the arg specs …" | [`K4`](n-ripgrep-2957/cases/K4.md) | suggestion, as graded |

## Why the owner decided these

These are `candidate` decisions: is the cause claim the same fault as a known problem? Policy v3 lets agents settle a candidate only when the recommender and two blind assessors from another model family agree. The owner's rule is Claude-only, so the blind answers here come from the recommender's family under [`same-family-blind.v9.md`](../../../bench/grading/rulings/same-family-blind.v9.md). Such records always go to the owner and count nothing toward delegation. `ruling route` sent all four to the owner.

## Steps

- **Opening.** [`opening.json`](opening.json) pins policy v3 and the 22 closed rounds (`a2c3530be`). The owner's same-family permission and its limits ([`blind-limits.v1.json`](blind-limits.v1.json)) were saved in the same commit.
- **Cases and first answers.** The asking session wrote the four neutral case files from the saved reviews, the pinned commits and `references.json`, and checked every quotation against its source. Its first answers are in [`recommender.v1.json`](recommender.v1.json), committed in `feda0a034` before any blind session. The blind assessors read [`assessors/rule.v6.md`](assessors/rule.v6.md), the round's copy of rule version 6. That copy is the rule text followed by the scoring rubric, because the rubric's first section holds the cause-and-result rule. They also read the candidate precedent sheet [`assessors/precedents-candidate.v1.md`](assessors/precedents-candidate.v1.md).
- **Dossiers.** The record check needs a dossier for each candidate. These questions establish no new fact. Each dossier copies, unchanged, the promise and delivery of the known problem it asks about, with their searches and saved responses, from the dossier that established them:
  - GT-p3: `regrade-v10-rulings-2026-10-17/p-hono-5067`, M1.
  - GT-v16: `candidate-rulings-2026-11-10/v-django-17914`, NC-0266123a5aa5.
  - GT-n3: `regrade-v10-rulings-2026-10-17/n-ripgrep-2957`, M1b.

  `summary.json` names each source in `promise_from`. `bench-tools ruling dossier` passes on all three folders.
- **Blind answers.** One Claude Opus 5.5 and one Claude Sonnet 5.5 session per case, at high effort, without fast mode: 8 sessions, $2.59 at list price ([`usage.v1.json`](usage.v1.json)), within the limit of 12 sessions and $5. Every session wrote a valid answer on its first run.
- **Records.** The before-records in [`rulings/`](rulings/) passed `ruling check` and were committed in `ddc24e67d`, before the owner was asked. The owner answered the four questions one at a time ([`answers.md`](answers.md)). The after-records and the round summary ([`summary.v1.md`](summary.v1.md)) closed the round in `8958546be`.

## The answers

| Question | Recommender | Blind Opus | Blind Sonnet | Owner |
| --- | --- | --- | --- | --- |
| 1 | duplicate GT-p3, medium | duplicate GT-p3, low | duplicate GT-p3, medium | duplicate GT-p3 |
| 2 | duplicate GT-p3, medium | duplicate GT-p3, low | suggestion, low | duplicate GT-p3 |
| 3 | duplicate GT-v16, medium | duplicate GT-v16, low | duplicate GT-v16, low | duplicate GT-v16 |
| 4 | duplicate GT-n3, low | suggestion, medium | suggestion, medium | suggestion |

Question 4 is a miss of cause `slip`. The case file showed the `_arguments` error at the base as well, and GT-n3's cause in the FAQ. The recommender still joined the fallback with that error and called it GT-n3. Both blind assessors weighed those facts. When asking, the session said it now agreed with them, and the owner kept the suggestion.

## Filing

Receipt [`cohort-rebuild.v24.md`](../../../bench/grading/rulings/cohort-rebuild.v24.md) files questions 1 to 3 as saved claims with approved eligibility decisions, each linked as related to its comment:

- `CL-p-round-trip-broader-than-needed`: GT-p3.
- `CL-p-fix-at-wrong-depth`: GT-p3.
- `CL-v-configure-reads-creator-wrapper`: GT-v16.

Question 4 matches its grade, so nothing is filed and the ripgrep batch stays current.

## Regrade

The filing made two batches stale: sonnet `p-hono-5067` (queue rebuilt) and builtin-selected `v-django-17914` (queue selected). The plans, the authorizations and `start.sh` are in [`regrade/`](regrade/). The owner's answer 2 of 2026-11-20 ("1 yes 2 yes") approved a cap of up to $26. The run is capped at $20 across the two queues: each queue's Opus price plus the $6 reservation, and a share of the rest. The expected spend is $5.42 at the batches' latest Opus grades.

| Queue | Batch | Cap | Expected | Spend at list price | Wall time (UTC, 2026-11-20) |
| --- | --- | --- | --- | --- | --- |
| rebuilt | sonnet `p-hono-5067` | $8.73 | $1.86 | $3.23 | 17:05 to 17:19 |
| selected | builtin-selected `v-django-17914` | $11.26 | $3.57 | $3.50 | 17:19 to 17:34 |
| total | 2 of 2 mapped | $20.00 | $5.42 | $6.73 | |

Hono cost $1.37 more than its latest Opus grade. Both batches mapped on the first try.

## What changed

Compared with the grades before the filing, for the filed comments:

- **Question 1** (sonnet Hono `att-007` item-2). The cause sentence is now graded with its results. c4 has GT-p3 and c5 has GT-p7, both problems, and no cause-only claim is left. c4's reason: "cause and result one thought, as ruled for CL-p-round-trip-broader-than-needed".
- **Question 2** (sonnet Hono `att-029` item-1). c3 is a problem with GT-p3: "one thought with the claim field, as ruled for CL-p-fix-at-wrong-depth".
  - The claim list now splits out the consequence's "The extra rewrite is what causes the cache regression above." as its own claim, a known-cause suggestion ("Only restates the cause").
  - That is the shape of #14 again, a cause and "the regression". Here, though, the regression is named only by pointing at another comment. It is left for the owner to raise.
- **Question 3** (builtin-selected Django `att-015` item-4). The claim field and its stale-time-zone result are now one problem claim, c10, with GT-v16 credit.

Known-problem credit at review level changed in one place: sonnet Hono `att-007` gained GT-p4. The regrade credited a separate item (item-3, "Handlers using `validator('form')` and `c.req.parseBody()` on the same request can then see different results.") with GT-p4, GT-p5 and GT-p8. `att-008` and `att-019` gained a second GT-p1 claim without the cause, and their GT-p1 credit is unchanged. Django's known-problem credit is unchanged.

Other claims shifted as regrades do. The Django batch merged many claims: it went from 123 claims to 100. One Django claim is now unresolved as a blocked check: `att-015` item-8, whether Django's tests fail with the pool on, which needs a PostgreSQL server. That claim moves `src/lib/metrics.test.ts` from 15 unresolved claims to 16.

## Checks

- `current-grading check`: 615 of 615 admitted reviews assessed.
- `claims check`: passes.
- `evaluator-audit check`: the four whole-cohort checks pass.
- `calibration check`: passes, and `decision-queue.json` is unchanged.
- `bun run verify:import`, `build:site`, `typecheck` and `test:web`: pass.
- The archives are under `bench/regrading/audit-r4-reach-2026-11-20-{rebuilt,selected}/`.
