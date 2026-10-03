# Detail: OverlappingFieldsCanBeMergedRule argument comparison

File: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts` (826 lines at head; no file-size concern).

## Commands and measurements

- `git diff main...review-head`: one file, 15 insertions, 27 deletions. Imports `ArgumentNode` and `ValueNode` were dropped and `ObjectValueNode` added.
- `./node_modules/.bin/mocha src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts`: 46 passing.
- Read `src/utilities/sortValueNode.ts`: it sorts `OBJECT` fields by `naturalCompare` on name, recurses through lists and objects, and returns scalars unchanged.

## What changed

Before: `sameArguments(args1, args2)` compared lengths, then for each argument in the first list used `find` on the second by name and compared `print(sortValueNode(value))`. `stringifyValue` was a one-line wrapper. After: `stringifyArguments(node)` wraps all arguments in one `ObjectValueNode` and returns `print(sortValueNode(...))`; `findConflict` compares the two strings.

## Behavioural equivalence

- Argument order independence: preserved, since the object fields are sorted by name.
- Nested value sorting: preserved, via the same `sortValueNode`.
- Different argument counts or names: produce different strings, so still reported as conflicting.
- Duplicate argument names: previously the first `find` match won; now both entries are printed. Such documents are already rejected by `UniqueArgumentNamesRule`, so this is not reachable in valid operation.
- Edge: `naturalCompare` ordering of argument names is deterministic and symmetric, so two equal sets always print identically.

Verification status: equivalence reasoned from reading; the existing suite passes. No new test was added in this commit; the order-insensitivity tests from the preceding commit (#3455) still exercise the path.

## Finding 1 detail: lost fast path

Old cost for two argument-less fields: one length comparison and an empty `every`. New cost: two object literals, two empty `map`s, two `sortValueNode` spreads with an empty sort, two `print` calls (visitor-based printer) and a string comparison. The rule's own comments note fragments are compared many times, so per-comparison overhead is multiplied. Verification status: reasoned from the code only; not benchmarked.

Worked code-judo proposal, keeping the single flow but restoring the cheap exit:

```ts
const args1 = node1.arguments ?? [];
const args2 = node2.arguments ?? [];
if (args1.length !== args2.length || stringifyArguments(args1) !== stringifyArguments(args2)) { ... }
```

or, without extra branching, memoize the printed string per `FieldNode` in a `WeakMap` so each node is stringified once however many times it is compared.

## Finding 2 detail: synthetic node

`ArgumentNode` and `ObjectFieldNode` are structurally identical for `name` and `value`, which is what makes the trick work, but nothing in the code says so. Proposal: add to `src/utilities` a `sortArguments`-style helper next to `sortValueNode`, or at least comment the intent. This keeps AST fabrication out of the validation layer. Verification status: judgement, not a defect.

## Finding 3 detail: fallback placement

The `?? []` with `c8 ignore next` and the FIXME for #2203 moved from the call site into `stringifyArguments`. Behaviour is unchanged. Remove both when the type is fixed upstream. Verification status: judgement, not a defect.
