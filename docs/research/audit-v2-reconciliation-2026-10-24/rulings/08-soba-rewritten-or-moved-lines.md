# Ruling 08-soba-rewritten-or-moved-lines

Round `audit-v2-reconciliation`. Case file: [`cases/08-soba-rewritten-or-moved-lines.md`](../cases/08-soba-rewritten-or-moved-lines.md).

Decisions in this question: `u45` (reconciliation), `u59` (reconciliation).

## `u45`: unit #45, `2026-10-08-last-push-claude-fable/att-029#c8`

Record: [`08-soba-rewritten-or-moved-lines.u45.before.json`](08-soba-rewritten-or-moved-lines.u45.before.json)

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
- recommender names a gap in the rule: Ground rule 2 exempts a line rewritten only to rename something, and says nothing of a line removed and re-added with its commands merged and its packages re-sorted.
- recommender found no earlier ruling of this shape
- no rule of this decision type has been tested blind
```

## `u59`: unit #59, `2026-10-10-last-push-claude-thermo-sonnet/att-019#c3`

Record: [`08-soba-rewritten-or-moved-lines.u59.before.json`](08-soba-rewritten-or-moved-lines.u59.before.json)

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
- recommender names a gap in the rule: Ground rule 2 decides by added, removed or rewritten lines, and does not say whether unchanged lines become the change's when it wraps them in a new function.
- recommender found no earlier ruling of this shape
- no rule of this decision type has been tested blind
```
