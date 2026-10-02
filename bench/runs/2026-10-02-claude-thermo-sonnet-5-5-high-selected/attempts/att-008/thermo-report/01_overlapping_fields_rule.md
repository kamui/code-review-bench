# Detail: OverlappingFieldsCanBeMergedRule.ts

## Measurements and commands

- `git diff main...review-head --stat`: 1 file, +15/-27.
- `wc -l src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`: 826 lines (no threshold concern).
- `./node_modules/.bin/mocha src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts`: 46 passing.
- `sortValueNode` (src/utilities/sortValueNode.ts) copies each object's fields and sorts them with `naturalCompare`; it was introduced in #3455 and is also used by `findBreakingChanges.ts`.

## Finding 1: unconditional allocation and printing in findConflict

Old flow: `args1`/`args2` defaulted to `[]`, then `sameArguments` returned false on a length mismatch, and otherwise did a `find` per argument and compared per-value printed strings. For two argument-less fields the work was two empty-array fallbacks and a length comparison.

New flow: `stringifyArguments` is called twice per compared pair. Each call builds an `ObjectValueNode`, one `ObjectFieldNode` per argument, goes through `sortValueNode` (a second copy plus sort) and then `print`. The cost is paid even when both lists are empty, and even when the argument counts differ.

Worked code-judo proposal: keep the single helper but make it return the cheap answer first, for example `function sameArguments(a, b)` guarding `if ((a.arguments?.length ?? 0) === 0 && (b.arguments?.length ?? 0) === 0) return true`, and only then comparing `stringifyArguments`. This keeps the one-concept design from the PR and removes the common-case cost. A larger move is a `WeakMap<FieldNode, string>` memo, since each node participates in many pairs. Verification: static reasoning only, not benchmarked.

## Finding 2: implicit encoding trick

The arguments are encoded as an input-object value to reuse `sortValueNode`. This is a good reuse of the canonical helper (it deletes the bespoke per-argument matching), but the intent is undocumented, the name `stringifyArguments` suggests display formatting, and the FIXME / `c8 ignore` for the optional `arguments` field now sits inside the helper. Remedy: comment plus rename; no structural change needed. Verification: read-only.

## Behaviour equivalence check

- Order of arguments: normalised by sorting object fields (covered by the existing test at test lines 239-257).
- Nested object order: still handled recursively (test lines 259-280).
- Count mismatch: yields different strings.
- Duplicate names: invalid per `UniqueArgumentNamesRule`; stable sort keeps the author's order, and the old code compared by first match, so neither is meaningful for invalid input.
