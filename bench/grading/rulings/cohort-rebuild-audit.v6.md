# Rulings on the evaluator audit during the issue 30 cohort rebuild, sixth receipt

Saved human ruling of 2026-10-30 on the [evaluator audit](../../../docs/evaluator-audit.md), made after the sample of the rule-version-4 grades was drawn with seed `evaluator-audit-v2-2026-10-29` (691 units in 170 batches, commit `798894857`) and before any second assessment of it. The entry holds the question, the options shown and the option the owner chose, as the session that asked relayed them.

## Audit decision 10: the second grader of round evaluator-audit-v2-2026-10-29

Asked 2026-10-30 at about 2:30 PM US Eastern (about 18:30 UTC). The plan names Codex GPT-6.1 Sol at high effort as the second grader. Sol graded every batch the round sampled, so it is not independent of the first grader in this round. The background shown came from [the proposal](../../../docs/research/rule-v4-regrade-2026-10-25/second-pass-proposal.v1.md): the 691 items in 170 batches, why Opus, about $190 expected ($170 to $240), the $275 cap with a $6 limit per session, a 5-batch trial first, about 12% to 24% of a Claude week, starting after Saturday's 5 AM ET reset, and the other Claude work planned that week.

Question as shown: "Use Claude Opus 5.5 high as the second grader for the 691-item audit sample, with a $275 cap, starting after Saturday's 5 AM ET Claude reset (~$190 expected, ~12–24% of a Claude week)?"

Options shown: "Yes, Opus, $275 cap (Recommended)", "Yes, but smaller cap", "Not now".

The user chose "Yes, Opus, $275 cap (Recommended)".

Decision: Claude Opus 5.5 (`claude-opus-5-5`) at high effort is the second grader of round `evaluator-audit-v2-2026-10-29`, on its 170 sampled batches, with at most $275 at list-price equivalent on the Claude plan, reruns included, and at most $6 per session. It starts after the Claude week resets on 2026-10-31 at 5 AM ET. The plan's strata, sizes, tolerances and seed are unchanged. A later round whose first grader is not Sol uses the second grader the plan names, unless the owner rules again.

## Recorded with the decision

These follow from the decision; the owner was not asked about them.

- New launches stop at 90% of the Claude week, the stop the owner set for Claude grading on 2026-10-23 ([`quota-approval.v1.json`](../../../docs/research/rule-v3-regrade-2026-10-23/quota-approval.v1.json)). The approval is saved as [`approval-second-pass.v1.json`](../../../docs/research/rule-v4-regrade-2026-10-25/approval-second-pass.v1.json).
