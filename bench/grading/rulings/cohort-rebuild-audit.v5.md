# Rulings on the evaluator audit during the issue 30 cohort rebuild, fifth receipt

Saved human ruling of 2026-10-29 on the [evaluator audit](../../../docs/evaluator-audit.md), made after every selected batch was graded again under rule version 4 ([PR #409](https://github.com/kamui/code-review-bench/pull/409)) and before any sample of those grades was drawn. The entry holds the question, the option the owner chose, that option's text and the answer, as the session that asked relayed them. The other options shown were not relayed and are not recorded here.

## Audit decision 9: the round on the rule-version-3 grades is closed as superseded

Asked 2026-10-29 at about 4:15 PM US Eastern (20:15 UTC). The round drawn with seed `evaluator-audit-v2-2026-10-12` sampled the rule-version-3 grades (`grades.json` with SHA-256 `095a47810994c3828a75ade53a7a6c1188141898384f47de77276c2e0d73e519`) and was assessed a second time. Its reconciliation was paused on 2026-10-25 with question Q1 decided and questions Q2 to Q29 not asked. The rule-version-4 regrade replaces every grade it sampled.

Question as shown: "The old grading check (paused at Q2–Q29) is on the v3 grades, which v4 replaces. What should happen to it?"

Option chosen: "Close it as replaced (Recommended)": "Record that the v3 check is superseded by v4 and stop it. No more Q2–Q29 questions. Lets v4 merge once the new sample is drawn."

The user chose "Close it as replaced (Recommended)".

Decision: the round drawn with seed `evaluator-audit-v2-2026-10-12` is closed as superseded by the rule-version-4 regrade. It did not pass and is not described as passed. Questions Q2 to Q29 of its reconciliation are not asked. Its sample, second assessments, comparison and reconciliation records stay as evidence. The next sample is drawn from the rule-version-4 grades under the same plan.

## Recorded with the closure

These follow from the decision; the owner was not asked about them.

- The plan's strata, sizes and tolerances are unchanged. A new draw needs a seed of its own, because the round records live under `audit/<seed>/` and are written once, and `bench-tools calibration check` refuses a plan that shares a superseded round's seed. The asking session recorded the seed `evaluator-audit-v2-2026-10-29` before any sample of the rule-version-4 grades was drawn. The owner did not choose the name.
- The reconciliation's records are on branch `t3code/issue-30-audit-v2-reconcile` at commit `7db7cfe339450100b523d5b2fadc286f013b64bc`, unmerged and unchanged: question Q1 (unit 23, decided `first-error`) and the prepared questions Q2 to Q29. Merging them would add a ruling round that can never close, because Q2 to Q29 will not be answered.
