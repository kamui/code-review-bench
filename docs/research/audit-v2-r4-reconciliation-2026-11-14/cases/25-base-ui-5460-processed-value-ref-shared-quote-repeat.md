# Case: Base UI 5460: a claim whose quote holds both the `processedValueRef` sentence and the focus fix (round 3's question 19, graded again by the second grader)

Round `audit-v2-r4-reconciliation-2026-11-14`, evaluator audit `evaluator-audit-v2-2026-10-29-round-4`. This file is neutral: it gives the facts and both graders' answers and makes no recommendation. Each fact is marked **run** (executed, output saved under `probes/`), **read** (seen in source, a diff, the rubric or a saved record) or **reported** (someone's statement, not checked here). Facts are judged on what a reviewer could know at the cut-off; later records only confirm.

Units in this case: #08. "The first grader" made the grade under audit (Claude Opus 5.5, high effort, rule version 6). "The second grader" is the audit's independent grade (Claude Sonnet 5.5, high effort). The first grader's rule is `docs/research/audit-v2-r4-reconciliation-2026-11-14/rule/rubric.first.md` (sha256 `4a680252…`), the composition its receipts pin; the second grader read the audit's composition built from `bench/rubric/scoring.safety-second.v6.md` (the receipt's `rubric.md`, sha256 `a0dfb723…`, the same as round 3's second grade).

## The same claim was decided in round 3

This unit is the same claim of the same review that round 3 reconciled as unit #16, question 19 (`docs/research/audit-v2-r3-reconciliation-2026-11-09/cases/19-base-ui-5460-processed-value-ref-shared-quote.md`). That case gives the pull-request body, the diff, GT-r9, last-push ruling 6, the three saved claims and the review item; this file does not repeat them.

The first grade is the same file: `bench/grading/current/assessments/2026-10-08-last-push-claude-fable/r-base-ui-5460/assessment-8/receipt.json` has sha256 `a1cd7c7a2d7d0b29…` now and in round 3's case, and git shows one commit for that directory (`594cb97aa`, 2026-11-08). The unit's first result, answers and reason are byte-identical in both rounds' `comparison.json`, and so is the claim split (`c5` focus, `c6` the shared quote, `c7` `clearErrors`) (**read**). Only the second grade is new, and its result is the same as in round 3: **problem**, `c6` in one duplicate group with `c5` for GT-r9.

The owner decided on 2026-11-09 (`docs/research/audit-v2-r3-reconciliation-2026-11-09/rulings/19-base-ui-5460-processed-value-ref-shared-quote.u16.after.json`, **read**):

```json
{
 "contract": 2,
 "ruling": "19-base-ui-5460-processed-value-ref-shared-quote",
 "before": {
  "path": "docs/research/audit-v2-r3-reconciliation-2026-11-09/rulings/19-base-ui-5460-processed-value-ref-shared-quote.u16.before.json",
  "sha256": "7b85acda69e3a2b26d3674548058391be0b03829e996e33196b0cdc4d26305d0"
 },
 "settled_by": "owner",
 "policy": null,
 "asked": 1,
 "outcome": "first-correct",
 "by_default": false,
 "ground": "agreed with recs",
 "miss": null,
 "lesson": null
}
```

The reason filed in `bench/grading/current/audit/evaluator-audit-v2-2026-10-29-round-3/reconciliation.json` (**read**): "The owner decided first-correct. The first grader made the split and graded c6 as the processedValueRef point, with the focus point in c5, and that point is a minor defect by last-push 6, which is what it gave. The second grader, seeing only the quote, graded the focus words again as a duplicate of c5's GT-r9 credit; round 1 question 03's owner ruling kept the first grader's definition of such a claim."

## What has changed since (**read**)

- The rule text is the same file (`cmp` with round 3's `rule/rubric.first.md`: identical).
- The rulings index gained four rulings after round 3 opened (`candidate-rulings-2026-11-10` 1 and 2, `soba-scope-candidates-2026-11-10` 1 and 2); none concerns Base UI. `claims.json` and `references.json` gained no Base UI entry.
- The second grader again saw only each item's ids and quotes, not the first grader's notes (its `claims.json` in `bench/regrading/audit-v2-r4-2026-11-11/2026-10-08-last-push-claude-fable/r-base-ui-5460/attempt-1/evidence.tar.gz` holds `kind`, `quotes` and `ids` per item).
- The second grader's ground is round 3's in substance. Round 3: "Same focus point as c5 (duplicate group). The processedValueRef half of this quote is a separate point … and does not add credit for the focus problem." Now: "Repeats c5 for the focus point and is one duplicate group for credit. The processedValueRef half of the sentence is a separate point … and is not counted toward GT-r9." The group is named `dup-focus-fix` instead of `d1`.
- For contrast, in the same batch the review att-030 (`blind-81109d`) makes the same two points, and its claim list isolates them: `c6` (focus, problem) and `c7` "and a `processedValueRef` keystroke-dedupe guard, but neither appears in the diff" (minor defect). Both graders agree on those.

## The unit

### Unit #08: `2026-10-08-last-push-claude-fable/att-020#c6`

- Unit type: claim; stratum: minor-defect; configuration `claude-builtin-fable-high`; attempt att-020 of run `2026-10-08-last-push-claude-fable`.
- The first grader's result: **minor-defect**; the second grader's result: **problem** (GT-r9, duplicate of `c5`).
- First grade receipt: `bench/grading/current/assessments/2026-10-08-last-push-claude-fable/r-base-ui-5460/assessment-8/receipt.json` (sha256 `a1cd7c7a2d7d0b29…`; review token `blind-3f93d6`); second grade: `bench/grading/current/audit/evaluator-audit-v2-2026-10-29-round-4/second/2026-10-08-last-push-claude-fable/r-base-ui-5460/assessment-1/verdicts.json` (sha256 `6f7047b02d4e83ef…`; token `blind-9ddfc5`).

Review item-2 (verdicts key `3`) is quoted in round 3's case 19. Claims the first grader listed for this item: `c5` "a fix so a control that unmounts or becomes disabled while focused no longer latches `data-focused`, but neither change exists in the diff; the focus latch bug is still present." (first **problem**, second **problem**, both GT-r9); `c6` "The PR body claims a `processedValueRef` guard and a fix so a control that unmounts or becomes disabled while focused no longer latches `data-focused`, but neither change exists in the diff" (first **minor-defect**, second **problem**); `c7` "Likewise the body says this PR does not call clearErrors(name) on prop changes, but line 110 does; reviewers relying on the description get the wrong picture of merged behavior." (first **minor-defect**, second **minor-defect**).

The first grader's claim record, with its notes and evidence:

```json
{
 "id": "c6",
 "quote": "The PR body claims a `processedValueRef` guard and a fix so a control that unmounts or becomes disabled while focused no longer latches `data-focused`, but neither change exists in the diff",
 "true": "yes",
 "this_change": "yes",
 "promised": "yes",
 "promise_source": [
  "announced"
 ],
 "delivered": "yes",
 "outcome": "minor-defect",
 "kind": null,
 "known_problems": [],
 "pre_existing_problems": [],
 "pre_existing_candidate": null,
 "canonical_claim_id": null,
 "duplicate_group": null,
 "candidate": null,
 "open": null,
 "notes": "Graded for the processedValueRef half (the focus half is c5). No processedValueRef exists in the diff. The outcome the description names (a controlled keystroke is not validated twice) arrives by another design: the controlled change handler returns early and only the value-sync block validates. Ground rule 3: a description sentence the code contradicts while the outcome arrives is a minor defect.",
 "evidence": [
  "git diff 30b8ea2..HEAD: no processedValueRef",
  "packages/react/src/field/control/FieldControl.tsx:141-146",
  "claims.md CL-r-description-processed-value-ref (related): minor defect"
 ]
}
```

The second grader's claim record, with its notes and evidence:

```json
{
 "id": "c6",
 "quote": "The PR body claims a `processedValueRef` guard and a fix so a control that unmounts or becomes disabled while focused no longer latches `data-focused`, but neither change exists in the diff",
 "true": "yes",
 "this_change": null,
 "promised": null,
 "delivered": null,
 "promise_source": [],
 "outcome": "problem",
 "kind": null,
 "known_problems": [
  {
   "family": "GT-r9",
   "says_what": "yes",
   "identifies_cause": "yes",
   "reason": "Repeats c5: the described fix that a control disabled/unmounted while focused no longer latches data-focused 'does not exist in the diff'."
  }
 ],
 "pre_existing_problems": [],
 "pre_existing_candidate": null,
 "canonical_claim_id": null,
 "duplicate_group": "dup-focus-fix",
 "candidate": null,
 "open": null,
 "notes": "Repeats c5 for the focus point and is one duplicate group for credit. The processedValueRef half of the sentence is a separate point (description's account of the design is untrue while a keystroke is still validated once: minor defect, claims.md CL-r-description-processed-value-ref) and is not counted toward GT-r9.",
 "evidence": [
  "packet.md section 3",
  "git diff 30b8ea2..HEAD: no processedValueRef and no Focused change"
 ]
}
```

## The question

The same as round 3's question 19: is `c6`'s label the minor defect of the `processedValueRef` point the first grader defined it as, or a problem (a duplicate of `c5`'s GT-r9 credit) because its quote says the focus fix is missing? Does the owner's decision on unit #16 carry to this second grade of the same first grade?
