# Second pass of evaluator audit round evaluator-audit-v2-2026-10-29-round-2

Round `evaluator-audit-v2-2026-10-29` failed. The rule-version-5 regrade then graded every batch again with Claude Opus 5.5 at high effort, and round 2 drew 721 units in 202 batches ([`sample.json`](../../../../bench/grading/current/audit/evaluator-audit-v2-2026-10-29-round-2/sample.json)). The owner chose Claude Sonnet 5.5 at high effort as the second grader, with a $170 list-price cap, random workspace names and one retry per failed batch ([audit decision 12](../../../../bench/grading/rulings/cohort-rebuild-audit.v8.md)). The draw came after the site published the rule-version-5 scorecard, a deviation the owner accepted ([audit decision 13](../../../../bench/grading/rulings/cohort-rebuild-audit.v8.md)). [`authorization.second-pass.v2.json`](../authorization.second-pass.v2.json) pins these limits, the plan, the runtime and the files below, and [`preflight-second-pass.v1.json`](../preflight-second-pass.v1.json) is the no-charge preflight.

## Files

The scripts are the [round 1 pass](../../rule-v4-regrade-2026-10-25/second-pass/README.md)'s, with these changes:

- [`second_pass.py`](second_pass.py):
  - The grader is `claude-sonnet-5-5`. The plan still prices each batch by its latest `claude-opus-5-5` first grade.
  - Preparation and preflight read the second host's cache root, `.local/regrade-v5-2/dependency-cache`, which the plan records as `cacheRoot`.
  - The 11 j-trpc-5017 batches provision from the trpc archive rebuilt on this host, through [its manifest](../../trpc-cache-rebuild-2026-11-04/README.md) ([audit decision 14](../../../../bench/grading/rulings/cohort-rebuild-audit.v9.md)).
  - State is under `.local/second-pass-v5`, and archives go to `bench/regrading/rule-v5-regrade-2026-11-03-audit-r2`. Workspaces keep their random names.
- [`launch.py`](launch.py) retries a failed batch at most once.
- [`start.sh`](start.sh) runs the launcher in this checkout under a lock, with no start time.
- [`plan.v1.json`](plan.v1.json) lists the 202 batches in sample order: 126 rebuilt, 11 trpc, 36 selected and 29 with no cache manifest. Their latest Opus first grades total $231.62.
- [`authorize.v2.py`](authorize.v2.py) writes the authorization. Version 1's launcher stopped at its first meter reading, because the meter reports an unused five-hour window as null. Version 2 reads it as zero use. No batch started under version 1.
- `test_second_pass.py` and `test_launch.py` exercise both scripts against fakes, with no model calls: `python3 -I <file>`.

The pass runs in this checkout. `grades.json` must stay the file the sample pins until the pass ends, so no batch is filed meanwhile.

Status, without a model call:

```sh
python3 -I docs/research/rule-v5-regrade-2026-11-03/second-pass/launch.py status
tail -f .local/second-pass-v5/launcher.log
```
