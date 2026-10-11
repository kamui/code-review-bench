# First live round under policy v2

## Withdrawn duplicate queue

Do not resume the six carry groups from this round. On 2026-10-11 the owner identified that the other session had already decided them. The original answers in that session and its committed ruling files confirm all six. The branch is `t3code/issue-30-last-push-rulings`; it had not merged when this round was prepared. The preparation used the merged carry inventory without checking that branch.

The [withdrawal receipt](reconciliation/withdrawal.json) pins the source commit, preserved ruling copies, all 34 affected saved decision IDs and this round's unchanged before-record. [Owner answer excerpts](reconciliation/owner-answer-excerpts.json) retain the original attributed responses from session `3955286c-7e9e-412b-97de-630142b83c8c`.

| Carry groups | Existing decision | Saved source |
| --- | --- | --- |
| r2 | GT-r8 stands as a problem, other-material, with both the error and dirty mark | [Ruling 26](reconciliation/04-base-ui-ruling-26.md) |
| r3, r5 | Prevented-event claim stays a minor defect | [Ruling 20](reconciliation/02-base-ui-ruling-20.md) |
| r4 | Sanitized-value claim stays a suggestion | [Ruling 24](reconciliation/03-base-ui-ruling-24.md) |
| r7 | Rejected-keystroke claim stays a suggestion; no credit for GT-r5, facts no/no | [Ruling 30](reconciliation/05-base-ui-ruling-30.md) |
| s4 | GT-s3 stays one serious problem covering both sites | [Ruling 12](reconciliation/01-seaweedfs-ruling-12.md) |

The owner chose "Stands as saved (Recommended)" for GT-r8 at 21:01:33 UTC, before this round's opening commit at 21:19:44 UTC. The duplicate question in this session is withdrawn. No answer is needed here. These existing decisions must not be counted again as new decisions, confidence matches or a qualifying live round for issue #59. Integration and publication belong to the existing source branch.

## Preserved preparation and accounting

Policy v2 was adopted and merged in [PR #123](https://github.com/kamui/code-review-bench/pull/123). The unchanged [opening snapshot](opening.json) records the history this preparation used. It is retained as evidence of the incomplete discovery, not revised after the fact.

The neutral r2 case, source evidence, bounded search inventory, prior execution evidence, recommender answer and two authorized blind assessments remain under `cases/r2/`. Their reported cost was $0.665184 against the approved $4 cap. Both blind calls completed before the overlap was discovered. No more assessments are needed for this queue.

The [before-record](rulings/01-base-ui-r2.before.json) and raw answers remain unchanged. [Recording notes](cases/r2/recording-notes.md) explain the original mechanical conversions. There is no after-record attributing a new ruling to the owner. Existing summary tooling consequently still lists this before-record as pending; the withdrawal receipt above governs orchestration. Do not mistake that pending entry for a question to ask again.

Issue #59 remains open. Resume its qualifying-round work only after reconciling the latest closed rounds, unmerged ruling branches and current rule version. Use genuinely undecided work; this duplicate preparation supplies no qualifying settlement.
