# The regrade under the next rubric

Part of [issue 30](https://github.com/kamui/code-review-bench/issues/30). This directory holds the plan and the authorization for grading the published selection once under the rubric in force. [`plan.py`](plan.py) sizes the queues and, given the approved ceiling and the saved approval, writes each queue's pinned plans and authorization.

## What is graded

The selection published by [#132](https://github.com/kamui/code-review-bench/pull/132): 198 batches holding 663 saved reviews and 2,153 comments. 623 of the reviews are admitted. No batch has a grade.

| Queue | Tasks | Batches | Saved reviews | Comments | Allowances | Ceiling |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `selected` | `u` to `y` | 45 | 189 | 645 | $141.66 | $117.40 |
| `rebuilt` | `i` to `s` without `l` and `n` | 125 | 387 | 1,247 | $377.52 | $312.87 |
| `plain` | `l`, `n` | 28 | 87 | 261 | $84.12 | $69.71 |
| All | 16 | 198 | 663 | 2,153 | $603.30 | $499.98 |

A queue holds the batches that share one cache-replacement manifest. An allowance is what a session may spend; the sum bounds the cost of sessions that finish. A ceiling is the queue's share of the approved $500, by its allowances.

## Approval

[`approval.v1.json`](approval.v1.json) is the owner's answer of 2026-10-12: "Opus 5.5, $500 cap (Recommended)". The grader is Claude Opus 5.5 at high effort with a 45-minute session limit, as [grading readiness](../../../grading-readiness.md) says. The owner first asked whether Sonnet 5.5 or Haiku 5.5 could grade, and chose Opus after the [grader-model test](../../grader-model-test-2026-10-12/README.md).

The expected usage is about $337 at list price by the trial's fit of $1.31 a batch and $0.036 a comment. The test's fit for Opus gives $325. Both come from twelve batches that average 26 comments, against 11 here.

## How it runs

The owner starts the queues from a shell, so that no assistant session holds the grader's plan while they run. A pilot of one batch from each queue goes first:

```sh
LIMIT=1 nohup bench/tools/regrade_queues.sh run .local/regrade/logs 1 \
  docs/research/cohort-rebuild-2026-10-05/regrade/authorization.selected.v1.json=.local/regrade/selected \
  docs/research/cohort-rebuild-2026-10-05/regrade/authorization.rebuilt.v1.json=.local/regrade/rebuilt \
  docs/research/cohort-rebuild-2026-10-05/regrade/authorization.plain.v1.json=.local/regrade/plain &
```

After the pilot's three grades are inspected, the same command without `LIMIT=1` and with `3` workers grades the rest. `bench/tools/regrade_queues.sh status .local/regrade/selected .local/regrade/rebuilt .local/regrade/plain` reports progress from saved files.

A queue stops launching at 95% of a usage window of the plan, with a sign-in that expires within half an hour, or with meters it cannot read, and ends `quota-stopped` with exit 4. Running the same command again resumes it. A failed batch stops new launches and keeps its attempt; it is diagnosed before anything is graded again.

The grades stay on a working branch until the [audit's](../../../evaluator-audit.md) sample is drawn, so `grades.json` on `main` holds no batch until then.
