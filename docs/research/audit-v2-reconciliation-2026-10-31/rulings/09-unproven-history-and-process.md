# Ruling 09-unproven-history-and-process

Round `audit-v2-reconciliation-2026-10-31`. Case file: [`cases/09-unproven-history-and-process.md`](../cases/09-unproven-history-and-process.md).

Decisions in this question: `u05` (reconciliation), `u31` (reconciliation).

## `u05`: unit #5, `2026-09-28-sonnet-5-5-rebench/att-033#c15`

Record: [`09-unproven-history-and-process.u05.before.json`](09-unproven-history-and-process.u05.before.json)

`bench-tools ruling route`:

```text
ASK
- reconciliation stays on its existing owner or reconciler route
- two blind high-confidence answers and the medium-or-high recommender must agree
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

`bench-tools ruling check`:

```text
Stays with the user:
- recommender found no earlier ruling of this shape
- no rule of this decision type has been tested blind
```

## `u31`: unit #31, `2026-10-08-last-push-claude-opus/att-016#c5`

Record: [`09-unproven-history-and-process.u31.before.json`](09-unproven-history-and-process.u31.before.json)

`bench-tools ruling route`:

```text
ASK
- reconciliation stays on its existing owner or reconciler route
- recommender: gap or conflict in the rule
- two blind high-confidence answers and the medium-or-high recommender must agree
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

`bench-tools ruling check`:

```text
Stays with the user:
- recommender names a gap in the rule: The rule does not say how much a claim about why an earlier release shipped a bug needs beyond the missing test and the bug it would have caught.
- recommender found no earlier ruling of this shape
- no rule of this decision type has been tested blind
```
