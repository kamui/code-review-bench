# Policy v3 terms confirmed

The owner confirmed the drafted terms of [ADR-0008](../../../docs/adr/0008-settle-adjudications-on-agreement.md) on 2026-11-12, shown at commit `5f5c8e789`. This confirms the draft. It is not the adoption: policy v3 stays `proposed` until the owner adopts one exact commit after enforcement is complete, with the receipt `delegation.v3.md`.

## The gap and conflict term, 2026-11-11

The owner was shown two replays of the saved decisions. With policy v2's rule that any stated gap or conflict sends the decision to the owner, 65 of 353 decisions pass the gates and 63 stand. Without it, 100 pass and 98 stand. The options were:

- Drop the rule and list each flagged gap in the round digest (recommended).
- Keep the rule.

### Owner's answer

> I agree with your recommendation.

## The terms as shown, 2026-11-12

The owner was asked to reply "accept as drafted" or to name the numbers to change.

What agents would settle:

1. Reconciliation: the reconciler's call at medium or high confidence. Five calls per audit round still come to the owner without the reconciler's call.
2. Impact label: the recommender and the inspector give the same label. A label is prepared only after the problem is admitted.
3. Candidate: the recommender and both blind assessors give the same outcome, and name the same known problem for a duplicate.
4. Pre-existing admission: the recommender and both blind assessors agree. An agreed `admitted` adds a pre-existing problem without the owner's ruling; 21 of the 23 that pass in the replay are `admitted`.

What always stays with the owner:

5. A new problem, a relied-on use, any unproven or cannot-tell answer, and any review of a saved ruling.
6. Credit, grouping, control, pre-existing raise and pre-existing grouping decisions.

How trust works:

7. Entry bar: a type is open in a round when at least 15 of the owner's decisions on at least 4 pull requests passed its gate in earlier closed rounds, and at least 90% of them stand.
8. A miss: when the owner changes a settlement, that type is suspended for the rest of the round and its other settlements in the round are listed for the owner. The change counts against the type at the next round opening, instead of clearing the count.
9. What the owner reads: every settlement of a type, one line each, in the first round it is open; one in five in later rounds. A settlement counts toward the bar only when the owner confirms it.
10. Model change: after any party's model is replaced, the owner reads every settlement of that type for one round.

Already decided:

11. A stated gap or conflict no longer holds an agreed answer; the digest quotes each one.

### Owner's answer

> accept as drafted
