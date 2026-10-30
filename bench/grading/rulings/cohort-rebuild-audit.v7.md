# Rulings on the evaluator audit during the issue 30 cohort rebuild, seventh receipt

Saved human ruling of 2026-10-30 on the [evaluator audit](../../../docs/evaluator-audit.md), made after round `evaluator-audit-v2-2026-10-29` was drawn and before its second pass started. The entry holds the question, the option the owner chose, that option's text and the answer, as the session that asked relayed them. The other options shown were not relayed and are not recorded here.

## Audit decision 11: new batches may join the grades while round evaluator-audit-v2-2026-10-29 runs

Asked 2026-10-30 at about 4:30 PM US Eastern (20:30 UTC). The plan says: "From the draw to the conclusion `grades.json` must stay the file the sample pins." The round drew 691 units in 170 batches from `grades.json` with SHA-256 `9d4a1fa9683f783b7c21ec4cb11bfaf1070e5b2f6c7cd67c119f9c8cb350ca44`. Its second pass by Claude Opus 5.5 was set to start on Saturday 2026-10-31 at 5:10 AM US Eastern. Grading the thermo-nuclear-review benchmark ([#213](https://github.com/kamui/code-review-bench/issues/213)) adds new batches to `grades.json` before the round concludes.

Before asking, the owner said: "Great, let's aim to finish the grading and publishing tonight".

Question as shown: "Push thermo (#213) to publish tonight? It needs your exception: new batches may join the grades file while the v4 check runs (the check keeps its own pinned copy). Est. 6–20 points of the Claude week (73% now), live roughly 1–3 AM ET."

Option chosen: "Yes, exception, thermo tonight (Recommended)": "Small tool change + ruling record; parallel claims intake, Opus grading of 60 thermo batches, publish. Stop new work if the Claude week reaches 92%. New problems show as unresolved until you rule."

The user chose "Yes, exception, thermo tonight (Recommended)".

Decision: round `evaluator-audit-v2-2026-10-29` keeps its own copy of the grades it was drawn from, as `bench/grading/current/audit/evaluator-audit-v2-2026-10-29/grades.json`, with the SHA-256 the sample pins. Every later step of the round reads that copy, so new batches may be filed in `bench/grading/current/grades.json` before the round concludes. The audited population stays exactly the drawn one: 691 units in 170 batches. The plan's strata, sizes, tolerances, seed and second grader are unchanged. The exception covers this round only.

## Recorded with the exception

These follow from the decision; the owner was not asked about them.

- The new batches are outside the round. They are not sampled, and a pass of the round does not describe them.
- Replacing a grade of one of the 170 sampled batches stays outside the exception: the plan corrects confirmed errors only after reconciliation. The round's steps refuse a live `grades.json` whose sampled batches differ from the copy. Thermo grading adds batches and replaces none of the 170.
- The thermo grading stops new paid work when the Claude week reaches 92%.
