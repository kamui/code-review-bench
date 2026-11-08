# Rulings on the evaluator audit during the issue 30 cohort rebuild, eleventh receipt

A saved human ruling of 2026-11-08 on the [evaluator audit](../../../docs/evaluator-audit.md), made after every selected batch was graded again under rule version 6 and before round `evaluator-audit-v2-2026-10-29-round-3` was drawn. The entry holds the question, the options shown and the option the owner chose, with its text, as the session that asked relayed them.

## Audit decision 16: the spend of round evaluator-audit-v2-2026-10-29-round-3

Asked 2026-11-08 at about 6:30 PM US Eastern (about 23:30 UTC), through the question tool. [Audit decision 15](cohort-rebuild-audit.v10.md) set the plan after round 2's failure: after rule version 6 is filed and every batch is graded again under it, a third round is drawn with the seed `evaluator-audit-v2-2026-10-29-round-3`, with Claude Sonnet 5.5 at high effort as the second grader, under a separately approved spend. At the time of asking, the rule-version-6 grades, with the regrades after the candidate rounds of 2026-11-07 and 2026-11-08, covered 615 of 615 admitted reviews; no candidate was pending; the four whole-cohort checks passed for 194 selected batches (commit `dd1d8d2ad`). These grades are not on the main branch, and no scorecard or export has been built from them.

Question as shown: "Draw the audit sample now and run the second grader (Claude Sonnet 5.5 high, fast mode off)? About $105 at list price. I'd set a $130 cap and stop at 90% weekly use."

Options shown: "Yes, $130 cap (Recommended)", "Not now".

Option chosen: "Yes, $130 cap (Recommended)": "Draw the sample, run the second grader, then bring you any disagreements to reconcile. The PR waits until the draw is saved."

Decision: round 3 of the plan is drawn with seed `evaluator-audit-v2-2026-10-29-round-3` from the current grades. Its second grader is Claude Sonnet 5.5 at high effort, without fast mode, with a $130 list-price cap and a stop at 90% weekly Claude use. As in round 2 (audit decision 12), each second-pass workspace has a random name and a failed batch is retried once. The plan's strata, sizes, tolerances and selection rule are unchanged.
