# ce-code-review on Codex Sol and Astra under bwrap, 2026-11-26

This record freezes and runs the two ce-code-review Codex runs that the 2026-11-25 record of #544 (`docs/research/codex-missing-2026-11-25/README.md`) planned. It collects reviews only; grading is a separate round.

**Branch:** the runs were first frozen at `9ca9dcac0` on `feat/codex-ce-bwrap-sol-astra`, a branch stacked on #544. No review was dispatched from that freeze, and no queue pinned it. At the coordinator's request, #544 stays thermo-only. These runs moved to their own branch from `main` (`49962121a`) and were frozen again there; the earlier freeze is unused. The runtime crate is the same at both bases.

## Authority

The owner's instruction of 2026-11-26, relayed by the coordinator: "keep running the remaining tests until we use all Codex usage".

- It replaces the 90% weekly stop of 2026-11-25.
- It settles the Sol ce cost hold recorded in #544: $4.50 or more per attempt and a run cap of about $200, with 10 replacements.
- For Astra ce it asks for caps sized from the projection with about 1.3 times headroom.
- The account is on the ChatGPT Plus plan, so the dollar caps are bookkeeping at API list prices. The quota is the real limit.
- The owner's standing rule applies: "the queue should wait until session/weekly reset and resume".

Each manifest records this as the deviation `owner-instruction-2026-11-26`.

## Runs

| Run | Arm | Tasks | Trials | Run cap | Per attempt | Replacements | Queue |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `2026-11-25-codex-ce-sol61-high-bwrap` | `codex-ce-sol61-high-bwrap` | 16 | 48 | $200 | $6 | 10 | [`queue.codex-ce-sol-bwrap.v1.json`](queue.codex-ce-sol-bwrap.v1.json) |
| `2026-11-26-codex-ce-astra-high-bwrap` | `codex-ce-astra-high-bwrap` | 16 | 48 | $1,120 | $27 | 10 | [`queue.codex-ce-astra-bwrap.v1.json`](queue.codex-ce-astra-bwrap.v1.json) |

- **One run per model for both suites.** Each run's 16 tasks are the 11 of `rubric-v2-12-targets` and the 5 of `selected-prs-rubric-v2`, three trials a task. One run fills both roster lines of its model, as `2026-11-14-codex-ce-terra61-high-bwrap` did. The coordinator's list counted 48 trials for the first suite and 15 more for the second. The 48 trials already include the 15 selected-PR trials, so no separate selected-PR run is made.
- **Inputs:** the Sol run is the 2026-11-25 draft in #544 with its caps raised. The arm's `budget_usd_per_attempt` and the manifest's caps changed together before the freeze. The Astra run is the Sol run with the model changed, through `2026-10-02-codex-ce-astra-high`'s invocation and a new arm.
- **Sandbox and network:** both run under `bwrap-v1` with Codex's network off and the offline execution policy (`sol-ce-offline-v1`), so the two ce models share one cohort.
- **Order:** each run has its own queue and work root, so the Astra probe cannot run before the Sol run finishes. The controller runs the Astra thermo trials first, then Sol ce, then Astra ce.

### Sizing

| Basis | Value |
| --- | ---: |
| Sol ce, three filed 2026-10 attempts | mean $3.51, most $3.98 |
| Sol ce, 48 trials | about $168 |
| Astra to Sol cost on the same thermo tasks | 4.9× (2026-10-28), 5.3× (2026-11-25 selected); 5.1× used |
| Astra ce, one review | about $17.90 |
| Astra ce, 48 trials | about $860; × 1.3 = $1,120 cap |
| Astra ce, reservation | 5.1 × $3.98 × 1.3 = $27 |

Quota, from the 2026-11-25 thermo runs:

| Model | Weekly points per list-price dollar | 5-hour points per list-price dollar |
| --- | ---: | ---: |
| Sol | about 2 | about 13 |
| Astra | about 1.5 | about 9 |

So projections per review:

- **Sol ce:** about 7 weekly and 45 five-hour points. That is about 14 reviews a week, so the 48 trials take three to four weekly windows.
- **Astra ce:** about 27 weekly and 160 five-hour points. One review may need more than a 5-hour window holds. The controller stops for a ruling when a review is measured above what a window holds. The Astra probe shows whether a review can finish inside one window.

## Controller

`~/.t3/bench-runs-lanes/codex-controller-v3/` holds a throwaway controller kept outside the repository. It reads `phases.json` every loop and runs the phases in order: the thermo queue, then Sol ce, then Astra ce.

- **Weekly window:** it launches only while the weekly reading plus the run's largest measured weekly use per review stays at or below 100%, so a review never starts that cannot finish inside the window. Before a run's first measurement it uses an estimate. Otherwise it waits for the weekly reset and resumes.
- **5-hour window:** it uses the adaptive threshold, 95% less the largest measured use. Before a run's first measurement a phase sets its own guard. The ce phases start their probes on a nearly fresh window. Over the threshold, it waits for that window's reset and resumes.
- **Stops:** it stops after each ce run's setup probe for inspection, and on any attempt that is not valid, any block, or a review measured above what a window holds.

- **Checkouts:** each phase runs from the checkout of its own branch. The thermo queue runs from #544's branch, and the ce queues run from this branch's worktree.
