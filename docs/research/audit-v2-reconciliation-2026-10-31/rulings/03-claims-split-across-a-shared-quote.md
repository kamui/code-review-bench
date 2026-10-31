# Ruling 03-claims-split-across-a-shared-quote

Round `audit-v2-reconciliation-2026-10-31`. Case file: [`cases/03-claims-split-across-a-shared-quote.md`](../cases/03-claims-split-across-a-shared-quote.md).

Decisions in this question: `u06` (reconciliation), `u28` (reconciliation), `u08` (reconciliation).

## `u06`: unit #6, `2026-10-02-claude-builtin-fable-selected/att-007#c6`

Record: [`03-claims-split-across-a-shared-quote.u06.before.json`](03-claims-split-across-a-shared-quote.u06.before.json)

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
- recommender names a gap in the rule: No rule says whether a claim the first grader defined as one part of a shared quote is judged on that part or on the whole quote, which is all the second grader could see.
- recommender found no earlier ruling of this shape
- no rule of this decision type has been tested blind
```

## `u28`: unit #28, `2026-10-08-last-push-claude-opus/att-008#c6`

Record: [`03-claims-split-across-a-shared-quote.u28.before.json`](03-claims-split-across-a-shared-quote.u28.before.json)

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
- recommender names a gap in the rule: No rule says whether a claim the first grader defined as one part of a shared quote is judged on that part or on the whole quote, which is all the second grader could see.
- recommender found no earlier ruling of this shape
- no rule of this decision type has been tested blind
```

## `u08`: unit #8, `2026-10-02-claude-builtin-fable-selected/att-009`

Record: [`03-claims-split-across-a-shared-quote.u08.before.json`](03-claims-split-across-a-shared-quote.u08.before.json)

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
- recommender names a gap in the rule: No rule says whether a claim the first grader defined as one part of a shared quote is judged on that part or on the whole quote, which is all the second grader could see.
- recommender found no earlier ruling of this shape
- no rule of this decision type has been tested blind
```
