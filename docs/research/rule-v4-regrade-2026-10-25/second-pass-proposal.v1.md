# Proposal: the second grader for audit round evaluator-audit-v2-2026-10-29

Status: draft for the owner's approval. Nothing here is authorized, and nothing has been dispatched.

The round's sample is [`sample.json`](../../../bench/grading/current/audit/evaluator-audit-v2-2026-10-29/sample.json): 691 units in 170 batches, across all 16 tasks and 14 configurations, drawn on 2026-10-30 at commit `798894857`. Each fact below is labelled **(run)** when a command produced it for this proposal, **(read)** when it comes from a saved record, and **(estimate)** when it is worked out from those.

## Why not Sol

[The plan](../../evaluator-audit.md#the-second-grader) names Codex GPT-6.1 Sol at high effort, because Sol was not the first grader when the user chose it. In this round Sol is the first grader. The receipt of each of the 170 sampled batches in `grades.json` names `gpt-6.1-sol` at `high` effort **(run)**, so a Sol second pass would not be independent. The proposal is Claude Opus 5.5 (`claude-opus-5-5`) at `high` effort, a different model family. Opus graded these batches under rule version 3 on 2026-10-23 and 2026-10-24, and the rule-version-4 grades replaced those. No Opus session produced a grade the sample pins, which is what the plan asks of a second grader.

## Cost

| Measure | Value |
| --- | --- |
| Opus 5.5 high, last first grade of each of the 170 sampled batches (2026-10-23 and 2026-10-24, Claude Code 2.1.309) | $214.25 in all; $1.26 mean, $1.20 median, $3.10 most **(run, from the saved receipts)** |
| Last round's Sol second pass ($52.57, 167 batches) against Sol's first grades of the same batches ($59.89) | 0.88 of the first-grade cost **(run, from the saved receipts)** |
| This pass, list-price equivalent: $214.25 × 0.88 | about $190 central, $170 to $240 range **(estimate)** |
| Proposed cap, reruns included | $275 |
| Per-session limit (`--max-budget-usd`) | $6, about twice the costliest Opus first grade |

The second pass prepares with `--inventory`, which omits the safety instructions and output, so it should cost no more per batch than a first grade. The range allows for the claim list adding reading and for a few reruns. The driver launches a batch only while the settled spend, the limits of the running sessions and the next session's $6 limit stay under the cap, so the cap needs that much room above the expected total.

## Claude plan use

- Meter at 2:14 PM ET on 2026-10-30: 72% of the week, 2% of the five-hour window. The week resets on Saturday 2026-10-31 at 5:00 AM ET **(run, `bench-tools subscription-meters read`, no model call)**.
- No record measures the weekly meter against grading sessions. Two review runs give about 7 to 10 points of the week per $100 of list-price use: CE Fable on 2026-10-10 (about 10 per $100), CE Opus on 2026-10-28 (30% to 37% for at least $86) **(read, [claude-gap-benchmarks-2026-10-28](../claude-gap-benchmarks-2026-10-28/README.md))**.
- At that rate the pass takes about 12% to 24% of a Claude week **(estimate)**. Started at 72% on Friday afternoon, it would end at 84% to 96% and could hit the 90% stop before the weekly reset. Started after the reset, it ends near 12% to 24% plus any other use of the plan.
- The same record has a five-hour window filling at $120 to $190, so the pass may stop once for a five-hour window and resume after it.

## Timing

The 170 Opus first grades took 11.3 hours of session time, 4.0 minutes on average and 10.5 minutes at most **(run, from the receipts)**. With 3 workers and the clone setup, the pass needs about 5 to 8 hours of running, plus up to 5 hours if a five-hour window fills **(estimate)**. Started at about 5:15 AM ET on Saturday, it ends Saturday afternoon or evening ET.

## How it would run

The operator starts it from a shell under `nohup`, with `BENCH_RUNTIME_STORE` set to a store the operator owns. It never runs from an assistant session on the Claude plan.

The driver is a Claude version of the last round's [`second_pass.py`](../rule-v3-regrade-2026-10-23/second-pass/second_pass.py), placed in `second-pass/` beside this file with its offline test. That driver refuses any grader except Codex Sol, so it needs these changes and no others:

- the grader is client `claude`, model `claude-opus-5-5`, effort `high`, with the client version pinned when the queue starts (2.1.314 today **(run)**);
- the preflight runs `grading probe-claude`, and `grade dispatch` passes `--max-budget-usd 6` in place of `--allow-unbounded-codex`;
- the quota check reads the Claude meters and stops new launches at 90% of the week. The owner set that stop for Claude grading on 2026-10-23 ([`quota-approval.v1.json`](../rule-v3-regrade-2026-10-23/quota-approval.v1.json)). The driver then ends with exit 4, and running it again resumes;
- the plan lists the 170 batches from this round's `sample.json`, each with the cache replacement manifest its regrade queue used;
- the queue is under `.local/second-pass-v4/` and the archives are under `bench/regrading/rule-v4-regrade-2026-10-25-audit-v2/`.

Each batch runs `evaluator-audit claims`, `grade prepare --inventory`, `grade check-prepared --dispatching`, `grade dispatch`, `evaluator-audit second` and `regrade archive-attempt`, and never `grade map`, so `grades.json` stays the file the sample pins. The first run is a pilot of 5 batches (`--limit 5`, about $6), with the meters read before and after, to measure the weekly use per dollar before the rest launches.

## Draft receipt entry

To be added as `bench/grading/rulings/cohort-rebuild-audit.v6.md` once the owner answers, with the question and answer as asked:

> ## Audit decision 10: the second grader of round evaluator-audit-v2-2026-10-29
>
> Asked 2026-10-30 at about <time> US Eastern. The plan names Codex GPT-6.1 Sol at high effort as the second grader. Sol graded every batch the round sampled, so it is not independent of the first grader in this round.
>
> Question as shown: "<question>"
>
> Option chosen: "<option>": "<option text>"
>
> Decision: Claude Opus 5.5 (`claude-opus-5-5`) at high effort is the second grader of round `evaluator-audit-v2-2026-10-29`, on its 170 sampled batches, with at most $<cap> at list-price equivalent on the Claude plan, reruns included, and new launches stopping at 90% of the Claude week. The plan's strata, sizes, tolerances and seed are unchanged. A later round whose first grader is not Sol uses the second grader the plan names, unless the owner rules again.

## Draft change to the plan

In [`docs/evaluator-audit.md`](../../evaluator-audit.md#the-second-grader), after the sentence naming Sol:

> For round `evaluator-audit-v2-2026-10-29`, whose first grader is Sol, the user [selected](../bench/grading/rulings/cohort-rebuild-audit.v6.md) Claude Opus 5.5 at high effort instead.
