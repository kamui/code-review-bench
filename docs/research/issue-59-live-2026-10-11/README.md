# First live round under policy v2

Policy v2 was adopted by the owner and merged in [PR #123](https://github.com/kamui/code-review-bench/pull/123). This round starts from that merged commit. The immutable [opening snapshot](opening.json) pins both earlier closed rounds before any first answers are prepared.

Candidate decisions open at count 0, level 0. Credit decisions open at count 1 on one pull request, level 0. No decision type has earned delegation. Each question will be routed and shown to the owner one at a time.

## Live work

Begin with the six pending groups in [the saved-ruling carry inventory](../last-push-carry-2026-10-11/groups.v2.json), in order: Base UI r2, r3, r4, r5 and r7, then SeaweedFS s4. These groups cover 34 saved decision entries; they are not 34 independent questions or confidence matches. Prepare each actual decision separately and show related parts of a case together.

These cases review saved rulings, so they always require the owner. They can establish the first live use of the policy but cannot by themselves satisfy issue #59's majority-settled closing test. Do not replay settled cases to increase the count. New grading queues require their own concrete authorization.

## Accounting

The round is open. No before-record, live settlement or owner answer has been filed yet. Save the summary and digest here when the round closes, including each miss and its actual fix. Exclude reconciler calls and ADR-0006 settlements from both sides of the closing fraction. Issue #59 remains open unless more than half of the round's remaining decisions were settled under policy v2 and the owner changed none.
