# Synthesis note

Arena of two: A, this session's proposal as shown to the owner; B, Codex GPT-6 Astra high, working alone from the task brief. Judge: Claude Fable, blind to which was which.

## Pick

Base: **B**. Judge 28 of 30 for B, 19 of 30 for A. My own scoring agrees on every criterion except proportion, where I put them level.

Reasons: B defines populations on contract v2 fields, checks the credit derivation over the whole cohort before drawing (the cause of all six first-round credit errors), counts confirmed errors and not disagreements, and has a complete rule for failure, a second round and what may be said. A had gaps there, kept 20-unit strata whose pass rules out only 22%, and contradicted itself on whether the kind of suggestion is gated.

## Grafted

From A:
- The section layout and one table per section.
- The first plan's unit ids and selection sentence, which the tool already implements.
- The rough population table, labelled as estimates.
- `problem` claims audited through credit only: B's `label-problem` stratum dropped. Checked in `bench/rubric/grader.next.md`: a claim that says what goes wrong for a known problem is `problem`, so the stratum repeats `caught`.

From the judge's "what both miss", each checked in the repository:
- `other-material` credit stratum: `docs/current-grading.md` reports recall for the two labels separately.
- `control-silence` stratum: three audited-clean controls are in the selection (112 admitted reviews, 258 comments); `src/lib/scoring.ts` derives correct silence from claim labels.
- Claims linked to a saved ruling leave the label populations and get a whole-cohort check: the grader's instructions keep a linked claim's saved label. In the trial 106 of 493 claims were linked.
- The human receipt and the superseded state: `audits.json` pins receipts by path and SHA-256, and the schema's states are only `unassessed` and `assessed`.
- The expected reconciliation load and the self-repeat floor.

Mine:
- `serious` at 100 with one allowed instead of 60 with none: same limit, a 6% rather than 21% chance of failing a grader with a 0.4% error rate.
- A stratum audited whole has no tolerance. B's `floor(k × m / n)` gives 0 at 59 of 60.
- Cost from the trial's measured second-grader usage ($0.52 a batch), not the brief's assumption that it equals the first grader's. The judge's $380 followed my brief; the trial record says otherwise.

## Rejected

- A's 20-unit strata and its "undetermined counts against the tolerance".
- A's agreement report on the second fact: P24 says recorded, not audited.
- B's JSON-array unit ids and its hypergeometric bound as a second gate: one gate, the binomial limit.
- B's `label-problem` stratum.

## Verification

- Bounds recomputed: 0/60 4.87%, 1/100 4.66%, 1/60 7.66%, 0/30 9.50%, 2/60 10.12%, 1/40 11.32%, 1/30 14.86%, 1/20 21.61%.
- Populations counted from `bench/grading/current/`: 604 admitted reviews, 2,539 credit units (1,169 serious, 1,370 other-material), 2,056 review comments, 112 control reviews.
- Trial counts from `docs/research/cohort-rebuild-2026-10-05/trial/retest-final*/`: 493 claims (problem 175, suggestion 257, minor-defect 22, refuted 22, outside-this-change 8, unresolved 7, unproven 2), 318 comments, one unresolved credit unit.
- Not verified: the real populations (they exist only after grading), and how the contract v2 scoring port will map outcomes to the reliability counts.
