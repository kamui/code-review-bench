# Detail: OverlappingFieldsCanBeMergedRule argument comparison

Scope: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts` (15 insertions, 27 deletions), range `730d5af8..efdbbcfa`, inspected with `git diff main...review-head`.

## Measurements and commands

- File size is 826 lines at head, so the 1k-line threshold is not an issue. The diff shrinks the file by 12 lines.
- `./node_modules/.bin/mocha src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts` gives 46 passing.
- `sortValueNode` (`src/utilities/sortValueNode.ts`) sorts object fields with `naturalCompare`. I read `naturalCompare` to check whether two distinct names can compare equal, which would make the sort order-dependent and produce false "differing arguments" conflicts. Distinct names do not compare as 0, because the final `aStr.length - bStr.length` tiebreak, and the digit handling that stops after a leading `0`, separate names like `a1`/`a01` and `a0`/`a00`. No behavioral regression found.
- Duplicate argument names (`f(a:1, a:2)` against `f(a:2, a:1)`) still compare as different. That input is already rejected by UniqueArgumentNamesRule.

## What the change does well

The old `sameArguments` and `stringifyValue` pair (two helpers, a length check, a `find` per argument) becomes a single canonical-string comparison that reuses `sortValueNode`. This is the same canonicalization already used by `findBreakingChanges.ts`. Order-insensitivity falls out of the sort rather than a quadratic lookup. This is a real simplification, and the `ArgumentNode`/`ValueNode` imports go away.

## Finding 1 (low): the cheap fast path is gone, and both sides are rebuilt on every pair

Location: call site at line 591, helper at lines 637-650.

The old code returned at once when the argument counts differed, and for the very common zero-argument case it did no printing at all. The new code, for every pair of fields that is not mutually exclusive, builds a fresh `ObjectValueNode` for each side, copies and sorts the fields through `sortValueNode`, and prints both. A zero-argument field still allocates and prints `{}`. This runs inside `findConflict`, which is called once per candidate pair. A response name with n fields is compared O(n²) times, and every comparison rebuilds the same string for the same node.

This is not a correctness problem and probably not measurable on typical documents. It is the hot path of a validation rule that is known to be quadratic, and the rewrite removed the only early exit. Remedy options, simplest first. Keep a one-line fast path (`args1.length !== args2.length`, or both empty) in front of the stringify. Or memoize the canonical string per `FieldNode` in a `WeakMap`, so each node is stringified once per validation. The first option needs no new state.

Verification status: confirmed by reading the code. I did not benchmark it.

## Finding 2 (low): argument list is smuggled into a synthetic input-object node

Location: lines 637-650.

To reuse `sortValueNode`, the helper fabricates an `ObjectValueNode` (no `loc`) whose fields are re-wrapped `ArgumentNode`s. The arguments-as-object trick is concise but implicit. It relies on `ArgumentNode` and `ObjectFieldNode` having the same shape, on `sortFields` sorting by name, and on the printer's object formatting being stable. The variable name `inputObjectWithArgs` is the only hint. The `FIXME .../issues/2203` and `/* c8 ignore next */` for the optional `arguments` also now live inside a stringify helper, away from the comparison they affect.

This is a reasonable trade, and I would not block on it. Suggested tightening: add a short comment saying why the object wrapping is used (to get name-sorted canonical output from the existing utility). Alternatively, if this pattern is wanted elsewhere, expose a small helper next to `sortValueNode` instead of constructing AST nodes inline in a validation rule.

Verification status: confirmed by reading; behavior verified by the existing 46 tests, which pass.

## Code-judo assessment

The change is already the judo move: two helpers and a nested loop collapse into one canonical-string comparison built on an existing utility. The only further deletion available is making the optional `arguments` normalization unnecessary (the FIXME), which belongs to a separate AST-typing fix and not to this PR.

## Test coverage

No tests were added. Existing tests cover differing, added, missing and reordered arguments. A test for reordered nested object arguments is already covered through `sortValueNode`'s own tests. I do not consider missing tests a blocker.
