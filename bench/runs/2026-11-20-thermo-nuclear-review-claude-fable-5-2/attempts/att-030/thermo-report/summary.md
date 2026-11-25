# Thermo-nuclear review: graphql/graphql-js#3457

Range: `730d5af8e933235fd5aa312a00be465db0b8acf5..efdbbcfacd92adb78e002f2b8a61f2a6a1504c19` ("OverlappingFieldsCanBeMergedRule: simplify argument comparison"), one file changed: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`.

The change replaces `sameArguments(args1, args2)` (length check, then per-argument lookup by name and comparison of the printed, sorted value) with `stringifyArguments(node1) !== stringifyArguments(node2)`, where `stringifyArguments` wraps a field's arguments in a synthetic `ObjectValueNode`, sorts it with `sortValueNode` and prints it with `print`.

Two actionable findings. Both were reproduced by running the same script against the base tree (extracted with `git archive main` into the work directory) and the head clone, calling `validate(schema, doc, [OverlappingFieldsCanBeMergedRule])`. The existing rule tests (`OverlappingFieldsCanBeMergedRule-test.ts`, `sortValueNode-test.ts`) pass on head; neither finding is covered by them.

## Findings

### 1. [High] Every field-pair comparison now builds, sorts and prints an AST twice, making the rule 30–45× slower on argument-less fields

File: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, lines 591 and 637–650.

Evidence. `findConflict` runs for every pair of fields that share a response name (`collectConflictsWithin` / `collectConflictsBetween` are nested loops over the field lists, so N same-named fields give N·(N−1)/2 calls). On base, a pair of fields without arguments cost two `?? []` reads and one `length` comparison, and `print` was reached only when both fields had the same non-zero number of arguments. On head, line 591 calls `stringifyArguments` on both nodes unconditionally whenever the parents are not mutually exclusive. Each call allocates an `ObjectValueNode`, maps the arguments into `ObjectFieldNode`s, copies and sorts them in `sortValueNode`, then runs `print`, which is a full `visit()` traversal with the printer's reducer. This happens even when both fields have no arguments at all (the result is the constant `"{}"`), and nothing is memoised per `FieldNode`, so the same node is re-printed for every pair it takes part in.

Measured with the repository's own TypeScript loader, schema `type Query { f(a: Int, b: Int, ...): Int }`, rule run alone:

| Query | Size | Base | Head |
| --- | --- | --- | --- |
| `{ f f f … }` × 500 | 1 KB | 28.6 ms | 684.0 ms |
| `{ f f f … }` × 1000 | 2 KB | 84.7 ms | 2661.4 ms |
| `{ f f f … }` × 2000 | 4 KB | 267.3 ms | 10866.4 ms |
| `{ f f f … }` × 3000 | 6 KB | 544.9 ms | 24706.9 ms |
| `{ f(a: 1, b: 2) … }` × 500 | 7 KB | 1659.8 ms | 4824.4 ms |
| `{ f(a: 1, b: 2) … }` × 1000 | 14 KB | 6646.8 ms | 18445.7 ms |
| `{ f(a: 1, b: 2) … }` × 2000 | 28 KB | 27557.7 ms | 75151.2 ms |

All runs report zero validation errors on both revisions, so the extra time is pure overhead. Absolute numbers come from the Babel-register dev loader and will be lower in a built package, but the ratio is a property of the algorithm: roughly 30–45× for fields without arguments and about 2.8× for fields with arguments.

Impact. `OverlappingFieldsCanBeMergedRule` is in `specifiedRules`, so it runs synchronously on every untrusted document a server validates before execution. The quadratic pair count already existed on base; this change multiplies its constant by 30–45× for the most common field shape (no arguments). A 6 KB query that base validated in about half a second now blocks the event loop for about 25 seconds in this setup, and the cost keeps growing quadratically with query size. That turns an existing slow path into a cheap denial-of-service lever for any server that does not cap query size well below a few kilobytes, and it also slows legitimate large queries that repeat a response name through fragments. The commit presents itself as a simplification and gives no sign the cost was considered.

The regression is avoidable without losing the simplification: short-circuit when both fields have no arguments (and when the argument counts differ), or cache the stringified arguments per `FieldNode` (for example in a `WeakMap`) so each node is printed once instead of once per pair.

### 2. [Low] Argument order is no longer irrelevant when two argument names tie under `naturalCompare`, producing a false "differing arguments" error

File: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, lines 637–650 (the sort is reached through `sortValueNode` on line 649).

Evidence. Base matched arguments by exact name (`arguments2.find((argument) => argument.name.value === argument1.name.value)`), so argument order could never affect the result. Head relies on `sortValueNode` producing a canonical order, and that sort uses `naturalCompare` (`src/jsutils/naturalCompare.ts`), which is not a total order over distinct names: digit runs are accumulated into a JavaScript number (`aNum = aNum * 10 + aChar - DIGIT_0`), so two different runs that exceed 2^53 can parse to the same value, and with equal lengths the function returns `0`. `Array.prototype.sort` is stable, so tied names keep their source order and the two printed strings differ.

Reproduced with schema field `f(a9007199254740993: Int, a9007199254740992: Int): Int` and the document

```graphql
{
  f(a9007199254740993: 1, a9007199254740992: 2)
  f(a9007199254740992: 2, a9007199254740993: 1)
}
```

Base reports no errors. Head reports `Fields "f" conflict because they have differing arguments. Use different aliases on the fields to fetch both if this was intentional.`

Impact. A valid document whose two selections of the same field pass identical arguments in a different order is rejected. It only triggers when a field has two argument names that differ solely inside a digit run of 16 or more digits, so real-world exposure is very small; it is reported because it is a verified behaviour regression introduced by moving name matching onto a comparator that can tie. Comparing by exact name, or using a strict code-unit comparison as the tie-break for this purpose, removes it.

## Checked and not reported

- Duplicate argument names within one field now compare differently: `{ f(a: 1, a: 1) f(a: 1, b: 2) }` produced no overlap error on base (the old lookup found `a` twice and never looked at `b`) and produces "differing arguments" on head. The new answer is the more accurate one, and such documents are already rejected by `UniqueArgumentNamesRule`, so this is not a defect.
- Argument order, nested input-object field order, variables, lists, block strings versus quoted strings and missing `arguments` arrays behave the same as on base; value comparison still goes through `print(sortValueNode(...))` as before.
- The `ArgumentNode` and `ValueNode` type imports removed by the diff have no remaining uses in the file; `ObjectValueNode`, `Kind`, `print` and `sortValueNode` are all imported.
- No developer-experience, configuration, dependency or feature-gate changes are part of this diff.
