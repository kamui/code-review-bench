# Rulings on the evaluator audit during the issue 30 cohort rebuild, twelfth receipt

A saved human ruling of 2026-11-11 on the [evaluator audit](../../../docs/evaluator-audit.md), made after round `evaluator-audit-v2-2026-10-29-round-3` concluded as failed (`conclusion.json` in its directory). The plan says: "A second failure returns to the user for a new plan." The entry holds the question, the options shown and the owner's answer, as the session that asked relayed them.

## Audit decision 17: the plan after round 3's failure

Round 3 concluded as failed on 2026-11-10 at 02:15 UTC. The refuted stratum held 3 confirmed first-grader errors against its tolerance of 1, and the suggestion stratum held 2 against its tolerance of 2. Every other sampled stratum held none. The owner's calls on the 5 confirmed errors were filed as saved claims under receipt `cohort-rebuild.v21.md`, and the batches they reached were graded again. The Django grouping candidate that regrade raised was ruled on in round `candidate-rulings-2026-11-10`, filed under receipt `cohort-rebuild.v22.md`, and its batches were graded again.

On 2026-11-09 at 9:52 PM US Eastern (02:52 UTC on 2026-11-10), the session laid out the steps after round 3 in chat text. The last was: "Then decide whether to run another audit round. That costs about $80–105." The same figure was repeated after each regrade.

Asked in chat text on 2026-11-11 at 3:07 PM US Eastern (20:07 UTC), after the owner's "proceed":

> I can't tell which parked item "proceed" is for. One of them costs money and the other leads to a public release, so I haven't started either.
>
> 1. **Open the PR to main (Recommended).** I'd push the 39 commits to this branch, run `bun run verify:import` first, and open a PR. Opening it costs nothing and changes nothing on the site. The site only updates when the PR is merged, and I'd ask you again before that.
> 2. **Run audit round 4.** This is about $80–105 of paid grading. I'd plan it and bring you the exact cap before spending anything.
> 3. **Both:** the PR first, then plan the audit round.
>
> Which one should I do?

The owner answered "1 first" at 3:08 PM US Eastern. PR #454 was opened at 3:20 PM US Eastern. The owner then wrote, at 3:54 PM US Eastern (20:54 UTC): "do audit".

No other plan was shown: the owner was not offered changed strata, sizes or tolerances.

State at the time of the answer: every one of the 194 selected batches has a current grade, and 615 of 615 admitted reviews are assessed. No candidate is pending, and the four whole-cohort checks pass (merge commit `aa0a93d0f`). These grades are on the branch of PR #454, which is open and not merged. The session ran `bun run build` and the explorer tests on them while checking the merge with main; nobody opened the scorecard, and the PR has no preview deploy. The draw is fixed by the seed, so seeing the grades cannot change which units it takes.

Decision: the plan's strata, sizes, tolerances and selection rule are unchanged. Round 4 is drawn with the seed `evaluator-audit-v2-2026-10-29-round-4` from the current grades. Its second pass is a paid dispatch under a separately approved spend; the session brings the owner the second grader and the cap before any charge.

Round `evaluator-audit-v2-2026-10-29-round-3` concluded as failed on 2026-11-10. Audit decision 17 keeps the plan's strata, sizes, tolerances and selection rule and draws round 4 with the seed `evaluator-audit-v2-2026-10-29-round-4`.
