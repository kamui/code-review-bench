# Rulings on the evaluator audit during the issue 30 cohort rebuild, thirteenth receipt

A saved human ruling of 2026-11-12 on the [evaluator audit](../../../docs/evaluator-audit.md), made after round `evaluator-audit-v2-2026-10-29-round-4` was drawn and its no-charge preflight passed. The entry holds the question, the options shown and the owner's answer, as the session that asked relayed them.

## Audit decision 18: the spend of round evaluator-audit-v2-2026-10-29-round-4

[Audit decision 17](cohort-rebuild-audit.v12.md) drew round 4 with the plan unchanged and left its second grader and spend to a separate approval. The draw holds 682 units in 169 batches (commit `76c575060`). The no-charge preflight passed all 169 batches (`docs/research/audit-v2-r4-2026-11-11/preflight-second-pass.v1.json`).

Asked in chat text on 2026-11-11 at 7:59 PM US Eastern (00:59 UTC on 2026-11-12):

> Round 4 is ready to grade, and the free check passed on all 169 batches. The draw is committed (`76c575060`). Nothing has been charged yet.
>
> The paid part is the second grader, which re-grades the 682 sampled items so its answers can be compared with the first grades. I'd use the same setup as round 3: Claude Sonnet 5.5 at high effort with fast mode off, a stop at 90% weekly use, and one retry for each failed batch.
>
> - **Cost:** about $79 at list price. These batches' first grades total $179.04, against round 3's $176.76, and round 3's pass cost $77.95.
> - **Time:** about 4–5 hours. It starts with a 5-batch trial that projects the full cost.
> - **Weekly Claude use:** 27%. Round 3 used about 4 points.
> - **Sign-in:** expires at about 10:20 PM ET. If the pass starts now, it will pause then and pick up again once you sign in. Signing in first avoids the pause.
>
> Which should I do?
>
> 1. **Run it with a $130 cap, the same as round 3 (Recommended).** That leaves room for retries.
> 2. **Run it with a $105 cap,** the top of the range I quoted earlier.
> 3. **Not now.**

The question was repeated in shorter form on 2026-11-12 at 6:13 AM, 12:22 PM and, last, 1:38 PM US Eastern (18:38 UTC):

> 1. **Spend cap:** $130 (Recommended), $105, or not now. Once you approve, I'll need a new Claude sign-in.
> 2. **Round 4 scripts:** keep them as Python for this round (Recommended), or port them to Rust first.

The owner answered at 1:40 PM US Eastern (18:40 UTC):

> 1. go with rec
> 2. if they are throwaway, keep them python. If not, then port to rust

The round 4 scripts are committed as the record of how the pass ran, so they are not throwaway, and the second pass runs from a Rust port. The owner's answer on the spend does not depend on the runner's language.

State at the time of the answer: PR #454, which holds the draw and the preflight, was merged to `main` as `bf0f6aba9`. `grades.json` is the file the round 4 sample pins. No second-pass batch has been dispatched and nothing has been charged.

Decision: round 4's second grader is Claude Sonnet 5.5 at high effort, without fast mode, with a $130 list-price cap and a stop at 90% weekly Claude use. As in rounds 2 and 3, each second-pass workspace has a random name and a failed batch is retried once. The plan's strata, sizes, tolerances and selection rule are unchanged.
