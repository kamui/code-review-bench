# Second pass of evaluator audit round evaluator-audit-v2-2026-10-29-round-4

Round `evaluator-audit-v2-2026-10-29-round-3` failed on the refuted stratum. The owner's calls on its 5 confirmed errors were filed as saved claims (receipt `cohort-rebuild.v21.md`), and the batches they reached were graded again. So were the Django batches after the pool rulings (receipt `cohort-rebuild.v22.md`). The owner then chose to run round 4 with the plan unchanged ([audit decision 17](../../../../bench/grading/rulings/cohort-rebuild-audit.v12.md)). [`../open.py`](../open.py) closed round 3 in `audits.json` and set the seed. `bench-tools evaluator-audit draw` then drew 682 units in 169 batches ([`sample.json`](../../../../bench/grading/current/audit/evaluator-audit-v2-2026-10-29-round-4/sample.json)). The second grader and the spend are not approved yet.

## Files

The scripts are the [round 3 pass](../../audit-v2-r3-2026-11-08/second-pass/README.md)'s, with these changes:

- [`second_pass.py`](second_pass.py): the plan also reads the queues of the two regrades since round 3, `audit-v2-r3-corrections-2026-11-10/regrade` and `candidate-rulings-2026-11-10/regrade`. Each sampled batch still has exactly one cache replacement manifest. State is under `.local/second-pass-r4`, and archives go to `bench/regrading/audit-v2-r4-2026-11-11`.
- [`launch.py`](launch.py), [`resume.py`](resume.py) and [`start.sh`](start.sh) change only the round, the state directory and the lock.
- [`plan.v1.json`](plan.v1.json) lists the 169 batches in sample order: 110 rebuilt, 27 plain, 24 selected and 8 trpc. Their latest Opus first grades total $179.04 (round 3: $176.76, with a Sonnet pass of $77.95).
- [`authorize.v1.py`](authorize.v1.py) starts from round 3's, but it refuses and writes nothing while its `APPROVAL` is unset. Round 3's approval (audit decision 16) does not cover this round. Once the owner's approval of the grader and the cap is saved as a ruling receipt, `APPROVAL` takes that receipt, the owner's words and the cap. A grader other than Claude Sonnet 5.5 also needs `second_pass.py` and this helper changed.
- `test_second_pass.py` and `test_launch.py` exercise both scripts against fakes, with no model calls: `python3 -I <file>`. `test_second_pass.py` also checks that `authorize.v1.py` refuses without the approval.

The pass runs in this checkout. `grades.json` must stay the file the sample pins until the round concludes, so no batch is filed meanwhile.

## Preflight

[`preflight-second-pass.v1.json`](../preflight-second-pass.v1.json) is the no-charge preflight, run with Claude Code 2.1.317 from 22:07 UTC on 2026-11-11 to 00:53 UTC on 2026-11-12. It passed all 169 batches: each claim list was written, each workspace was prepared and checked for dispatch, and each input fingerprint matches its grade. A first attempt from 21:06 UTC was stopped after 13 passed batches by the 1-hour limit of the session's background command, not by the script; its scratch directory was removed and the preflight run again detached.
