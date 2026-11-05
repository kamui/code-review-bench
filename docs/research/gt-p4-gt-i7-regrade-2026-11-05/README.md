# Regrade after the GT-p4 and GT-i7 widenings, 2026-11-05

Part of issue #30. Receipt [`cohort-rebuild.v18.md`](../../../bench/grading/rulings/cohort-rebuild.v18.md) filed rulings 1 and 2 of round [`owner-rulings-2026-11-04`](../owner-rulings-2026-11-04/README.md): GT-p4 (Hono 5067) now covers a parameter after `boundary`, and GT-i7 (requests 6667) an adapter whose `cert_verify` sets `conn.ca_certs`. That made the 20 `p-hono-5067` and 20 `i-requests-6667` batches stale. The owner chose on 2026-11-05 to regrade them now and the whole cohort again under rule version 6 later: "Regrade 40 now, full later" ("Regrade the Hono and requests batches now (~$53), and again under v6 later.", [`approval.v1.json`](approval.v1.json)). The rubric stays at rule version 5.

## Files

- [`plan.py`](plan.py): the rule-version-5 regrade's `plan.v3.py` for this regrade. After `bench-tools grade invalidate` removed the 40 stale grades (218 stay current), it wrote [`source-plan.full.v1.json`](source-plan.full.v1.json) and gave every batch the cache manifest of its last regrade queue. All 40 fall in one queue, `rebuilt`: [`source-plan.rebuilt.v1.json`](source-plan.rebuilt.v1.json) and [`execution-plan.rebuilt.v1.json`](execution-plan.rebuilt.v1.json), with [`cache-replacements.v2.json`](../provision-sandbox-cache-2026-10-22/cache-replacements.v2.json) and the cache root `.local/regrade-v5-2/dependency-cache`.
- [`authorize.py`](authorize.py) wrote [`authorization.rebuilt.v1.json`](authorization.rebuilt.v1.json): Claude Opus 5.5 at high effort on Claude Code 2.1.317, a $60 list-price cap set by the coordinating session against $52.64 at the batches' latest Opus grades, the controller's reservation of at most $6 a session, a 45-minute session limit and the 90% weekly quota stop.
- [`approval.v2.json`](approval.v2.json): the owner's later answer raising the cap to $95, for a follow-on queue if the first stopped at $60. The queue finished under $60, so no follow-on queue ran.
- [`preflight.v1.txt`](preflight.v1.txt): the no-charge `bench-tools grade preflight` of every batch, which passed for all 40.

## Run

`bench/tools/regrade_queues.sh run .local/gt-p4-gt-i7-regrade/logs 2 authorization.rebuilt.v1.json=.local/gt-p4-gt-i7-regrade/rebuilt`, started detached with `setsid nohup` at 00:51 UTC with Claude Code 2.1.317 first on `PATH`; the queue's `client.json` pins that copy.

| Measure | Value |
| --- | --- |
| Batches mapped | 40 of 40 (125 reviews); none failed, blocked or retried |
| Spend at list price | $53.85 of the $60 cap; $52.64 at the latest earlier grades |
| Wall time | 2.59 h, 2 workers |
| Weekly quota | 58% at the start |

Every workspace's clone and dependency cache was removed at mapping (`workspace-pruned.json` in each). The archives are under `bench/regrading/gt-p4-gt-i7-regrade-2026-11-05-rebuilt/`.

## What changed

- The two units the filing was for: `2026-10-10-last-push-claude-thermo-opus` `att-017` `c11` is caught for GT-p4, and `2026-10-24-thermo-nuclear-review-claude-fable` `att-017` `c3` is caught for GT-i7. Both were unresolved before. `2026-10-24-thermo-nuclear-review-claude-opus` `att-018` stays caught for GT-i7.
- Five other recoveries changed, on known problems the filing did not touch, as the grader read the same comments differently:
  - GT-p3, `2026-10-08-last-push-claude-fable` `att-018` `c7`: caught to missed (now "says what goes wrong: no").
  - GT-p5, `2026-10-08-last-push-claude-sonnet` `att-008` `c7`: unresolved to caught.
  - GT-p4, `2026-10-08-last-push-claude-sonnet` `att-007` `c7`: missed to unresolved. The review is not admitted, so its claim earns no recovery.
  - GT-i5, `2026-10-10-last-push-claude-thermo-opus` `att-001` `c2`: caught to missed. The grader read that clearing `check_hostname` alone does not stop hostname checks under urllib3 2.x.
  - GT-i5, `2026-10-24-thermo-nuclear-review-claude-sonnet` `att-036` `c3`: missed to caught. This is a similar comment, read the other way.
- No new candidate. Pending candidate `NC-6f7e0c93f8ec` (Hono, `validator('json')` with a space before `;`) was raised again in other words, and its claim text changed. Three candidates stay pending: `NC-6f7e0c93f8ec`, `NC-896d9ea6c530` and `NC-de9bfe6de5b8`.
