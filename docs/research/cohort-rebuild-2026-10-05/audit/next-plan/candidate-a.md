# Proposal A: the next evaluator audit

This is the proposal the session showed the owner on 2026-10-11, written out against the task's eight points. Its strata, sizes and tolerances are unchanged from what was shown.

## 1. Strata

Ten strata, named after the labels of the rubric in force. Two kinds of unit, as in the first plan:

- a **credit unit**: one admitted review paired with one known problem of its task;
- a **claim unit**: one claim in the first grader's verdict for one admitted review.

| Stratum | Unit | Population, from the first grader's saved grades |
| --- | --- | --- |
| `caught` | credit | the review is credited for the known problem (one of its claims has "says what goes wrong" yes for it) |
| `missed` | credit | no claim of the review has "says what goes wrong" yes or cannot-tell for the known problem |
| `unresolved-credit` | credit | no claim says yes and at least one says cannot-tell |
| `serious-reference` | credit | the known problem's approved label is `serious`, whatever the credit |
| `suggestion` | claim | outcome `suggestion`, any kind |
| `minor-defect` | claim | outcome `minor-defect` |
| `refuted` | claim | outcome `refuted` |
| `unresolved-claim` | claim | the claim is left unresolved |
| `unproven` | claim | outcome `unproven` |
| `outside-this-change` | claim | outcome `outside-this-change` |

A claim whose outcome is `problem` is audited through the credit strata, as eligible claims were in the first plan. A known problem with no approved label when the sample is drawn is outside `serious-reference`.

## 2. Sample sizes and selection

| Stratum | Rough population | Size |
| --- | ---: | ---: |
| `caught`, `missed` | large | 20 each |
| `unresolved-credit` | small | 20 |
| `serious-reference` | large | 60 |
| `suggestion` | about 1,760 | 40 |
| `minor-defect`, `refuted` | about 160 each | 20 each |
| `unresolved-claim` | about 65 | 20 |
| `unproven`, `outside-this-change` | about 30 and 15 | every unit |

About 265 units. The populations are scaled from the trial's 422 claims and are estimates.

- A stratum that holds no more units than its size is audited whole.
- Selection is the first plan's rule with a new seed, `evaluator-audit-v2-2026-10-12`: after every selected batch has a current grade and before a scorecard or export is opened, sort each stratum's units by the SHA-256 of `<seed>:<stratum>:<unit id>` and take the first units up to the size. A credit unit's id is `<run>/<attempt>#<known problem>`, a claim unit's is `<run>/<attempt>#<claim id>`.
- A unit drawn in two strata is assessed once and counted in both.

## 3. Tolerances

A disagreement in a credit stratum is a different credit result (caught, missed, unresolved) from the second grader. A disagreement in a claim stratum is a different outcome; for `suggestion` the kind is compared too.

| Stratum | Allowed disagreements | A pass rules out, at about 95% confidence |
| --- | ---: | --- |
| `caught`, `missed`, `unresolved-credit` | 1 of 20 | a disagreement rate above about 22% |
| `serious-reference` | 0 of 60 | a rate above about 5% |
| `suggestion` | 2 of 40 | a rate above about 15% |
| `minor-defect`, `refuted`, `unresolved-claim` | 1 of 20 | a rate above about 22% |
| `unproven`, `outside-this-change` | 1 | nothing beyond the units themselves: they are audited whole |

Tolerances count disagreements that reconciliation finds to be the first grader's error. A pass shows that the first grader's labels in that stratum are not badly wrong. It does not show that every label is right, and at 20 units it cannot tell a 5% error rate from a 15% one.

## 4. Reported without a tolerance

- **The second fact**, "identifies the cause as a fault". In the trial's rerun one grader repeated its own answer on it about four times in five, so a tolerance would fail on noise. Decision P24 keeps it recorded and not shown. Agreement on it is reported per stratum.
- **Agreement before reconciliation** in every stratum, and the confusion counts between outcomes.
- **Kinds of suggestion**, as a breakdown inside the `suggestion` stratum.

## 5. The second grader's task

Codex GPT-6.1 Sol at high effort, as in the first round. For each sampled batch it gets the grading packet the first grader had and the first grader's list of claims for each sampled review: the quotation and the claim's text, with no outcome, no fact and no credit. It labels each listed claim under the rubric and records the two facts against every known problem. It does not split comments again and does not add claims. Credit results are derived from its labels by the same code that derives the first grader's.

## 6. Reconciliation and consequences

- Every disagreement is reconciled as the first round's were: a reconciler reads both assessments and the evidence and records which grader was right or that it cannot be determined, with a record for each call. Undetermined counts against the tolerance.
- A stratum over its tolerance fails the audit for the measures that depend on it. The scorecard keeps those measures unavailable with the reason, and the owner decides between a rubric clarification with a regrade of the affected batches and publishing with the failure stated.
- Before the audit concludes, the scorecard says the audit is unassessed. An unfinished audit is never described as passed.

## 7. Cost

About 265 units. The first round's 216 units fell in 100 batches. With the second grader labelling a given list, a session is lighter than a first-grader session, so the pass is on the order of 100 to 130 sessions at or below the first grader's cost per batch: roughly $170 to $220 at list price, on the ChatGPT plan. It gets its own estimate and authorization before the draw.

## 8. What the owner must decide

1. The strata as listed. Recommended: as listed, with suggestions as one stratum.
2. The sizes and tolerances. Recommended: as listed.
3. The second grader. Recommended: Codex GPT-6.1 Sol, high.
4. That the second fact carries no tolerance. Recommended: yes.

## What it needs from the tools

`calibration.py` accepts the new strata names. `evaluator_audit.py` reads verdict contract v2, derives the credit result from the two facts, and prepares the second grader's packet from the first grader's claim list.
