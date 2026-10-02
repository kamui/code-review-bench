# Thermo-nuclear code quality review: graphql-js #3457

Range: `730d5af8..efdbbcfa`, one file changed (`src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, +15/-27). Detail: `01_overlapping-fields-rule.md`.

## Verdict

Approve, with two low-severity suggestions. This commit is a net simplification. It replaces the `sameArguments` and `stringifyValue` helpers, a length check and a per-argument `find` with one `stringifyArguments` helper that builds a canonical string through the existing `sortValueNode` utility. The file shrinks to 826 lines, so no size threshold is crossed. No new special-case branching is added to `findConflict`. The existing 46 tests in `OverlappingFieldsCanBeMergedRule-test.ts` pass. I found no behavioral regression, including a check that `naturalCompare` cannot make distinct argument names sort order-dependently.

## Findings

**Finding 1 (low): the cheap early exit is gone, and the canonical string is rebuilt for every pair.** In `OverlappingFieldsCanBeMergedRule.ts` the call at line 591 now runs `stringifyArguments(node1) !== stringifyArguments(node2)`, and the helper at lines 637-650 allocates an object node, copies and sorts its fields, and prints it, even when both fields have no arguments. The previous code returned immediately on differing argument counts and did no printing for empty argument lists. `findConflict` runs once per candidate pair, and a response name with n fields is compared O(n²) times, so each node is re-stringified many times. This is unlikely to matter on typical queries, but it is the hot path of an already quadratic rule and the rewrite removed its only fast exit. Keep a trivial length or empty check ahead of the stringify, or memoize the string per `FieldNode` in a `WeakMap`. The first option adds no state. This was confirmed by reading and not benchmarked. Full evidence is in `01_overlapping-fields-rule.md`.

**Finding 2 (low): the argument list is wrapped in a fabricated `ObjectValueNode` to reuse `sortValueNode`, with no comment explaining why.** The helper at lines 637-650 builds a loc-less object node from re-wrapped `ArgumentNode`s, relying on the argument and object-field shapes matching and on name-sorted printing being the canonical form. The `FIXME #2203` and `c8 ignore` for optional `arguments` also moved into this helper, away from the comparison they affect. The trade is acceptable and I would not block on it. Add a brief comment saying the wrapper exists to get sorted canonical output, or, if the pattern is wanted elsewhere, put a small argument-canonicalization helper beside `sortValueNode`. Details are in `01_overlapping-fields-rule.md`.

## Code-judo note

The commit already makes the main simplification: it deletes a helper pair and a quadratic lookup by leaning on an existing canonicalization utility. The remaining incidental complexity, the optional `arguments` handling behind the FIXME, belongs to a separate AST-typing fix and is not for this PR.

## Suggested remediation sequence

1. Restore a cheap fast path (equal-length and non-empty check, or a per-node memo) ahead of the stringify at line 591.
2. Add a one-line comment on `stringifyArguments` explaining the object-wrapping, or move the helper next to `sortValueNode` if it will be reused.
3. No test changes are required for this refactor.
