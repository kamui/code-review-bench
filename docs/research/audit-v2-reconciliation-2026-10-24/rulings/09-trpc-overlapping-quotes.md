# Ruling 09-trpc-overlapping-quotes

Round `audit-v2-reconciliation`. Case file: [`cases/09-trpc-overlapping-quotes.md`](../cases/09-trpc-overlapping-quotes.md).

Decisions in this question: `u01` (reconciliation), `u02` (reconciliation).

## `u01`: unit #1, `2026-09-24-builtin-baseline/att-039#c4`

Record: [`09-trpc-overlapping-quotes.u01.before.json`](09-trpc-overlapping-quotes.u01.before.json)

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
- recommender names a gap in the rule: The rubric splits comments into claims and the audit takes the claim list as given, but no rule says whether a label is judged against the claim's recorded scope or against every word of its quote.
- no rule of this decision type has been tested blind
```

## `u02`: unit #2, `2026-09-24-builtin-baseline/att-080#c6`

Record: [`09-trpc-overlapping-quotes.u02.before.json`](09-trpc-overlapping-quotes.u02.before.json)

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
- recommender names a gap in the rule: The rubric splits comments into claims and the audit takes the claim list as given, but no rule says whether a label is judged against the claim's recorded scope or against every word of its quote.
- no rule of this decision type has been tested blind
```
