# Thermo Nuclear Review: graphql/graphql-js#3457

Range reviewed: `730d5af8..efdbbcfa` (`git diff main...review-head`), one commit, "OverlappingFieldsCanBeMergedRule: simplify argument comparison". The only file touched is `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`.

## What the change does

The old `sameArguments(args1, args2)` first compared argument-list lengths, then for every argument in the first list looked up the same-named argument in the second list and compared `print(sortValueNode(value))` of the two values. The new code replaces this with `stringifyArguments(fieldNode)`: it wraps a field's arguments into a synthetic `ObjectValueNode` (one `ObjectFieldNode` per argument), runs `sortValueNode` on it so fields are ordered by `naturalCompare` on name, prints it, and compares the two printed strings.

## What I verified

- Focused tests pass at head: `src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts` and `src/utilities/__tests__/sortValueNode-test.ts` (48 passing).
- `tsc --noEmit`, `eslint --max-warnings 0`, and `prettier --check` on the changed file are all clean. The removed `ArgumentNode` / `ValueNode` type imports have no remaining users.
- The printer (`src/language/printer.ts`) has no context-dependent value printing, so printing an argument value inside a synthetic object yields exactly the text the old code produced for the standalone value. Block strings vs. ordinary strings, variables vs. literals, and nested objects/lists behave identically before and after (confirmed by running both trees side by side on the same documents).
- Duplicate argument names (`field(a: 1, a: 1)` vs `field(a: 1, b: 1)`) were wrongly considered identical by the old `find`-based lookup and are now correctly reported as differing. This is an improvement, and `UniqueArgumentNamesRule` rejects such documents anyway, so it is not a finding.

To compare behaviour I extracted the base revision's `src/` into the work directory and loaded both trees in one Node process through `@babel/register`, running `validate(schema, doc, [OverlappingFieldsCanBeMergedRule])` on the same inputs against both.

## Findings

### Finding 1 (Medium): Per-pair cost of the argument comparison rises roughly 30x for argument-less fields, slowing realistic fragment-heavy documents and widening an existing quadratic DoS surface

File: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, lines 591 and 637-650.

Evidence traced: `findConflict` is invoked once for every pair of same-response-name fields that `collectConflictsWithin` / `collectConflictsBetween` discover, which is quadratic in the number of such fields (every pair of fragments spreading the same field, every repeated field in a selection set). On the base revision, for the overwhelmingly common case of fields with no arguments, `sameArguments([], [])` returned `true` after a single length comparison. On the head revision, `stringifyArguments` is called twice per pair unconditionally, each call allocating a synthetic `ObjectValueNode`, running `sortValueNode` (a copy plus `Array.prototype.sort`), and running the full `visit`-based `print` reducer, just to produce `"{}" === "{}"`. There is no fast path and the old length check was dropped.

Measured with both trees in one process (Node 24, warmed up):

| Document | base | head | ratio |
| --- | --- | --- | --- |
| `{ field ×200 }` (no args) | 4 ms | 107 ms | 28x |
| `{ field ×500 }` (no args) | 20 ms | 672 ms | 34x |
| `{ field(a: 1, b: 2) ×500 }` | 1242 ms | 3300 ms | 2.7x |
| Relay-style: 60 fragments on `User`, each selecting `id __typename login name url avatarUrl`, against the repo's `benchmark/github-schema.graphql` | 7.2 ms/op | 68.1 ms/op | 9.5x |
| Introspection query against the same schema (the repo's own `validateGQL` benchmark shape) | 1.07 ms/op | 0.85 ms/op | unchanged within noise |

Failure caused: validation runs on every untrusted incoming operation. Documents that spread many fragments on the same type (the normal shape of Relay / Apollo colocated-fragment apps, where each fragment selects `id` and `__typename`) see validation time for this rule grow by roughly an order of magnitude, with the extra cost scaling with the square of the fragment count. For adversarial inputs the rule was already known to be quadratic (see #3442 in the base history, "Prevent Infinite Loop in OverlappingFieldsCanBeMergedRule"); this change multiplies the per-pair constant by about 30 for the cheapest attacker payload (`{ a a a a ... }`), so a payload that previously cost about 2 s of CPU now costs about a minute. The change is a pure refactor with no functional goal that requires paying this cost, so the regression does not look intended. Restoring a cheap early-out (compare argument counts first, and return early when both lists are empty) before falling back to the print-based comparison would remove the regression while keeping the simplification.

### Finding 2 (Low): Arguments whose names differ only in a 17+ digit numeric run are falsely reported as "differing arguments" when written in a different order

File: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, lines 641-649 (the synthetic object passed to `sortValueNode`).

Evidence traced: `sortValueNode` orders object fields with `naturalCompare` (`src/jsutils/naturalCompare.ts`), which accumulates each digit run into a JavaScript number (`aNum = aNum * 10 + digit`). Once a run exceeds about 16 significant digits, distinct runs collapse to the same double and `naturalCompare` returns `0` for two different strings. `Array.prototype.sort` is stable, so such names keep their source order, and the printed strings differ purely because of source order. The old code looked arguments up by exact name and was immune to this. Confirmed by running both trees on

```graphql
{ field(a99999999999999998: 1, a99999999999999999: 2)
  field(a99999999999999999: 2, a99999999999999998: 1) }
```

Base revision: no errors. Head revision: `Fields "field" conflict because they have differing arguments. Use different aliases on the fields to fetch both if this was intentional.`

Failure caused: a valid document is rejected by validation. The same `naturalCompare` tie already affected input-object field names after #3455 (in the base), so this change extends a pre-existing edge case from object keys to argument names rather than introducing a new class of bug. The triggering names are extremely unusual, hence Low priority. Comparing argument counts first and using an exact-name lookup (or a tie-break on the raw string when `naturalCompare` returns `0`) would eliminate it.

## Not findings

- Duplicate argument names are now handled more strictly (see above); that is a correctness improvement.
- Reordering of arguments, nested input objects, lists, variables, enums, and block strings all produce identical validation results on base and head.
- No devex, feature-gate, dependency, or build changes are involved.
