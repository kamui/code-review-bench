# Second pass of evaluator audit round evaluator-audit-v2-2026-10-12

The owner approved a second grader, Codex `gpt-6.1-sol` at `high` effort, on the 167 sampled batches of [`sample.json`](../../../../bench/grading/current/audit/evaluator-audit-v2-2026-10-12/sample.json), with at most $200 at list-price equivalent on the ChatGPT plan, reruns included ([`approval-second-pass.v1.json`](../approval-second-pass.v1.json)). This folder holds the plan, the driver that runs it and the driver's offline test. The preflight is recorded beside the approval in [`preflight-second-pass.v1.json`](../preflight-second-pass.v1.json).

## Why a driver

No maintained controller runs a second pass. `bench-tools regrade run` maps every graded batch with `grade map`, which replaces grades, and `grades.json` must stay the file the sample pins until the round concludes. It has no step that writes a claim list, prepares with `--inventory` or saves with `evaluator-audit second`. Its Codex mode also refuses a dollar cap (`budgetCapUsd` must be `null` with `codex-unbounded`). The earlier round used a script in its research folder, [`audit/second.py`](../../cohort-rebuild-2026-10-05/audit/second.py), which imports the deleted Python.

[`second_pass.py`](second_pass.py) replaces it. It runs only documented `bench-tools` commands, never `grade map`, and changes no maintained code.

## Plan

[`plan.v1.json`](plan.v1.json) lists the 167 batches in sample order, 706 units, with the cache replacement manifest each batch's regrade queue used:

| Regrade queue | Batches | Cache replacement manifest |
| --- | ---: | --- |
| rebuilt | 106 | `docs/research/provision-sandbox-cache-2026-10-22/cache-replacements.v2.json` |
| selected | 42 | `docs/research/selected-cache-rebuild-2026-10-02/cache-replacements.v1.json` |
| plain | 19 | none |

Paths:

- Records: `bench/grading/current/audit/evaluator-audit-v2-2026-10-12/second/<run>/<target>/assessment-<N>/`, written once by `evaluator-audit second`, as [the plan](../../../evaluator-audit.md#the-second-grader) says.
- Queue: `.local/second-pass-v2/queue/`, with `status.json`, `client.json`, `client-bin/codex`, `driver.lock` and `attempts/<run>/<target>/attempt-<N>/`. An attempt holds `claims.json` (the per-batch claim list), `key.json`, `reservation.json`, `audit.log` and `work`, a link to its workspace.
- Workspaces: `.local/second-pass-v2/workspaces/<random hex>`. The path names no run, arm or attempt, so preparation and dispatch accept it.
- Archives: `bench/regrading/rule-v3-regrade-2026-10-23-audit-v2/<run>/<target>/attempt-<N>/`, written by `bench-tools regrade archive-attempt`.

## What `run` does

`second_pass.py run --authorization A [--workers N] [--limit L]`, from the repository root under `mise exec --` with `BENCH_RUNTIME_STORE` set:

1. Refuses (exit 2) unless the authorization's pinned files still match: the plan, the budget receipt and every `runnerDeviations` entry. It also refuses unless the plan's sample, `grades.json` and manifests match, and `bench-tools runtime pin` prints the authorization's `runtimes`. A second driver on the same queue also exits 2.
2. On the first run it copies the installed `codex` into `client-bin/` and records its version and SHA-256 in `client.json`. Every later run and every launch checks the copy and refuses a version other than `grader.cliVersion`. Every `bench-tools` step runs with `client-bin` first on `PATH`.
3. Restart scan, in plan order. A batch with a saved `assessment-*/receipt.json` is done. If the latest attempt has a good receipt (exit 0, verdicts present, no access violation, priced) but nothing saved, it runs `second` and the archive again without dispatching. Any other unsaved attempt stops the run (exit 1) until someone inspects it and writes `disposition.json` in it. The batch then gets the next `attempt-<N>`, and the earlier charge stays in the total.
4. Before each launch it checks the runtime pin and the client copy, then the budget, then the quota:
   - **Budget.** The settled charge is each attempt's `usage.high` from `work/dispatch.json`, failed attempts included. An attempt with a reservation and no priced receipt counts at its `reserveUsd`. The one exception is an attempt that is not running and has neither `work/home` nor `work/dispatch.json`: dispatch creates `home` after its last check before the paid call, so that attempt counts as zero. A batch launches only when settled + outstanding + $1 per outstanding reservation + `reserveUsd` + $1 ≤ `budgetCapUsd`. If that does not fit while sessions are running, it waits for them. If nothing is running, it stops as `budget-stopped` (exit 3).
   - **Quota.** It reads the pinned client's meters with `bench-tools subscription-meters read`, reuses a reading for 5 minutes, and asks `subscription-meters stop` with `quotaStopPercent` (default 95). When the meters say stop or cannot be read, it launches nothing more, lets running sessions finish, and ends as `quota-stopped` (exit 4).
5. It writes `reservation.json` and starts a worker, which runs, logging to `audit.log`: `evaluator-audit claims`, `grade prepare --inventory` with the batch's manifest, `grade check-prepared --dispatching`, `grade dispatch --allow-unbounded-codex --timeout <grader.timeoutSeconds>` (no `--run`, so no run's `charges.jsonl` changes), `evaluator-audit second` (one at a time; it removes the clone), then `regrade archive-attempt`.
6. The first failed step, or a receipt that is not good, stops new launches. Running sessions finish and are saved, and the run exits 1. When settled charges plus reservations pass the cap after a session, the run stops (exit 3); that session's assessment is still saved, because saving spends nothing.
7. `--limit L` launches at most L new batches in this run and ends as `limit-reached` (exit 0).

Codex has no per-session dollar limit, so `reserveUsd` is an estimate held for each running session, not a limit. The cap is enforced between sessions. One session can cost more than its reservation; the earlier round's most expensive batch cost $1.39.

`status --plan P` calls no model. It prints saved and planned batches, the last state and reason, spend and unsettled reservations computed from the attempts, the pinned client, the last run's `wallSeconds` and free disk.

`preflight --plan P --cli-version V --out R` also calls no model. It runs `grading probe-codex` and one meter reading. For each run and manifest, it runs `grade preflight-inputs` and `grade preflight --allow-unbounded-codex` on the sampled targets. Then, one batch at a time, it runs `claims`, `prepare --inventory` and `check-prepared --dispatching`. It checks that the key's input fingerprint equals the sampled grade's and that the workspace's safety policy is `no-safety/v1`, then removes the workspace. A failing batch keeps its workspace for inspection. Preparing one batch at a time keeps one clone on disk; the provisioner's `BENCH_DISK_RESERVE_GIB` check (default 20) still applies.

`test_second_pass.py` runs the `run` loop against a fake `bench-tools` and `codex` in a temporary directory: completion, limit and resume, budget stop, quota stop, stop at the first failure and retry after a disposition, resave without a second dispatch, and refusal of a changed runtime.

## Cost

The earlier second pass (100 batches, Codex 0.160.0, same model, effort and price entry) cost $39.02: mean $0.39, median $0.37, maximum $1.39. Its cost tracks the batch's claim count (r = 0.77; $0.017 a claim). Today's 167 batches hold 3,170 claims (mean 19, median 13, maximum 110), against a mean of 23 then. The grader's fixed inputs are larger now (rubric 6 KB → 51 KB), but on the 39 batches graded under both, the first grader cost only 1.06× as much under the v3 rubric. That suggests cached input adds little.

| Basis | Estimate |
| --- | ---: |
| $0.017 a claim × 3,170 | $55 |
| Linear fit on claims ($0.20 + $0.0085 a claim) | $60 |
| $0.39 a batch × 167 | $65 |
| Fit × 1.1 for the v3 contract | $66 |

Expect $55 to $80. The $200 cap covers 2.5 to 3 times that, which leaves room for reruns. The earlier round used about 14 points of the weekly Codex allowance for $39, so $65 is about 23 points.
