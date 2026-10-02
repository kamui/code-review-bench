# Detail 01: argument comparison in OverlappingFieldsCanBeMergedRule

Subsystem: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, the only file in the change.
Range: `main...review-head` (`730d5af8..efdbbcfa`).

## What the change does

On `main`, `findConflict` read both argument arrays and called a local `sameArguments`, which compared lengths, then for each argument of the first field looked up the same-named argument of the second with `Array.prototype.find` and compared `print(sortValueNode(value))` for the two values.

On `review-head`, both helpers (`sameArguments`, `stringifyValue`) are deleted. A new `stringifyArguments(fieldNode)` wraps the field's arguments in a synthetic object value, sorts it and prints it, and `findConflict` compares the two strings:

```ts
// line 591
if (stringifyArguments(node1) !== stringifyArguments(node2)) {

// lines 637-650
function stringifyArguments(fieldNode: FieldNode): string {
  // FIXME https://github.com/graphql/graphql-js/issues/2203
  const args = /* c8 ignore next */ fieldNode.arguments ?? [];

  const inputObjectWithArgs: ObjectValueNode = {
    kind: Kind.OBJECT,
    fields: args.map((argNode) => ({
      kind: Kind.OBJECT_FIELD,
      name: argNode.name,
      value: argNode.value,
    })),
  };
  return print(sortValueNode(inputObjectWithArgs));
}
```

File size: 838 lines on `main`, 826 on `review-head` (`wc -l`, and `git show main:<path> | wc -l`).

## Finding 1: the canonical string is recomputed per pair

### Why the call site is hot

`findConflict` is called from the innermost position of two nested loops:

- `collectConflictsWithin` compares every field against every later field with the same response name, `n(n-1)/2` calls.
- `collectConflictsBetween` compares every field of one collection against every field of the other with the same response name, `n*m` calls.

Each call on `review-head` now performs, twice:

1. an array `map` that allocates one object per argument plus the wrapper object,
2. `sortValueNode`, which spreads the wrapper, maps and spreads every field again, recurses into nested lists and objects, and sorts,
3. `print`, which is `visit(ast, printDocASTReducer)`: the general AST visitor with its stack, key lookup and reducer dispatch, run on the freshly built copy.

The string produced depends only on `fieldNode`. Nothing caches it. The rule already carries two caches for exactly this kind of repeated work (`cachedFieldsAndFragmentNames`, `comparedFragmentPairs`), with comments explaining that memoization "can dramatically improve the performance of this validator". The argument string is the one repeated computation in the rule that has no cache.

### The lost fast path

`main`:

```ts
if (arguments1.length !== arguments2.length) {
  return false;
}
return arguments1.every(...)   // empty array: returns true immediately
```

For two fields without arguments this is two property reads, one comparison and an `every` over an empty array. On `review-head` the same pair builds, sorts and prints two empty objects to compare `'{}'` with `'{}'`. Fields without arguments that share a response name are the normal case for this rule (`id`, `__typename`, `name` repeated across fragments).

### Measurement

Scratch spec (placed as `src/validation/__tests__/zz-bench-test.ts` inside `git archive` exports of each revision under the work directory, not in the clone):

```ts
const schema = buildSchema(`
  type Query { a: String, f(x: Int, y: [Int], o: In): String, q: Query }
  input In { a: Int, b: [Int], c: In }
`);
// For each case: parse once, 3 warm-up validations, 7 timed validations,
// report median and min; validate(schema, doc, [OverlappingFieldsCanBeMergedRule])
const N = 300;
time(`${N} repeated arg-less fields`, `{ ${'a '.repeat(N)} }`, 0);
time(`${N} repeated fields, 2 scalar args`, `{ ${'f(x: 1, y: [1, 2]) '.repeat(N)} }`, 0);
time(`${N} repeated fields, nested object arg`,
     `{ ${'f(x: 1, o: {c: {b: [1, 2, 3], a: 1}, a: 2}) '.repeat(N)} }`, 0);
```

Command, run in each exported tree with `node_modules` symlinked to the clone's:

```
BABEL_DISABLE_CACHE=1 <clone>/node_modules/.bin/mocha src/validation/__tests__/zz-bench-test.ts
```

Results (median of 7, Node v24.21.0, 300 fields = 44,850 pairs, each case expects and gets 0 errors):

| case | `main` | `review-head` | proposal below |
| --- | --- | --- | --- |
| no arguments | 7.08 ms | 242.49 ms | 7.75 ms |
| two scalar arguments | 1,091.36 ms | 2,290.87 ms | 13.38 ms |
| nested input object argument | 4,730.39 ms | 5,885.09 ms | 25.69 ms |

Reading the table: the PR makes the no-argument case about 34 times slower and the with-argument cases 1.25 to 2.1 times slower. The with-argument cases were already poor on `main`, because `main` also printed inside the pair loop; that is the pre-existing half of the problem and the reason the memo is the right fix rather than a revert. The scaling is quadratic, so the gap widens with the number of same-named fields: an attempt at 1,500 fields did not finish the `main` run inside the five-minute command limit and was stopped.

### Worked code-judo proposal

Keep the PR's single primitive and its call site. Make the primitive honest about being a per-node key:

```ts
// The canonical argument string depends only on the field node, so it is
// computed once per node rather than once per compared pair of fields.
const stringifiedArguments = new WeakMap<FieldNode, string>();

function stringifyArguments(fieldNode: FieldNode): string {
  // FIXME https://github.com/graphql/graphql-js/issues/2203
  const args = /* c8 ignore next */ fieldNode.arguments ?? [];
  if (args.length === 0) {
    return '';
  }

  let stringified = stringifiedArguments.get(fieldNode);
  if (stringified === undefined) {
    const inputObjectWithArgs: ObjectValueNode = {
      kind: Kind.OBJECT,
      fields: args.map((argNode) => ({
        kind: Kind.OBJECT_FIELD,
        name: argNode.name,
        value: argNode.value,
      })),
    };
    stringified = print(sortValueNode(inputObjectWithArgs));
    stringifiedArguments.set(fieldNode, stringified);
  }
  return stringified;
}
```

Notes on the choices:

- A module-level `WeakMap` keyed by the AST node avoids threading a third cache parameter through the nine functions that already pass `cachedFieldsAndFragmentNames` and `comparedFragmentPairs`. The value is a pure function of the node, and the codebase already memoizes on object identity with `WeakMap` in `src/jsutils/memoize3.ts`. If the maintainers prefer per-validation scope, the same map can be created next to the two existing caches in `OverlappingFieldsCanBeMergedRule` and passed alongside them; the measurement would be the same.
- The empty string for "no arguments" cannot collide with a non-empty argument list, whose printed form always starts with `{`.
- The call site at line 591 is unchanged from the PR.
- Net effect against `review-head`: about eight added lines, no new concepts at the call site, and prints drop from two per pair to one per field node that has arguments.

Verification of the proposal: applied to a third exported tree; `OverlappingFieldsCanBeMergedRule-test.ts` (46 tests) and the benchmark spec pass together, 47 passing. Not type-checked with `tsc`, since only Mocha runs were inside the execution allowance.

A further step, not required for approval: the same memo would let the rule drop the `/* c8 ignore next */ ?? []` line once issue 2203 is resolved, because the helper is now the only place that reads `fieldNode.arguments`.

## Finding 2: arguments disguised as an input object literal

The synthetic node is typed correctly (`ObjectValueNode` annotation, no cast) and the printed form is an injective encoding: names are GraphQL names, string values are quoted and escaped by the printer, and `sortValueNode` sorts object fields at every depth, including the synthetic top level, which is what makes argument order irrelevant.

The problem is legibility, not correctness. Three facts are load-bearing and none is written down:

1. The argument list is wrapped in an object value so the existing `sortValueNode` handles top-level argument order with the same code that handles nested object field order.
2. `print` is used here as an equality key, not to produce text for a human or an error message.
3. Equality of the printed strings is the definition of "same arguments" for this rule.

`inputObjectWithArgs` names the mechanism. `stringifyArguments` names the output type. Neither names the purpose. Suggested comment, to sit above the function once it also holds the cache from finding 1:

```ts
// Returns a canonical key for a field's arguments: two fields have the same
// arguments exactly when their keys are equal. The arguments are wrapped in an
// object value so that sortValueNode makes both argument order and nested
// input object field order irrelevant.
```

I do not recommend replacing the wrapping trick with a hand-written sort and join. That would add a second canonicalization path next to `sortValueNode` for no gain. One sentence of explanation is the right size of fix.

## Behavior check: duplicate argument names

Scratch spec `zz-dup-test.ts`, schema `type Query { f(x: Int, y: [Int]): String }`, rule run alone:

| document | `main` | `review-head` | proposal |
| --- | --- | --- | --- |
| `{ f(x: 1, x: 1) f(x: 1, y: [2]) }` | 0 errors | 1 error | 1 error |
| `{ f(x: 1, y: [2]) f(x: 1, x: 1) }` | 1 error | 1 error | 1 error |
| `{ f(x: 1, y: [2]) f(y: [2], x: 1) }` | 0 errors | 0 errors | 0 errors |
| `{ f f(x: 1) }` | 1 error | 1 error | 1 error |

The first row is the only difference. `main` was order-dependent because `find` returned the first same-named argument of the second field; `review-head` is symmetric. Such documents already fail `UniqueArgumentNamesRule`, so the practical impact is confined to running this rule in isolation. Recorded as a question in the summary, not a finding.

## Checks with no finding

- File-size threshold: 838 to 826 lines, no concern.
- Imports: `ArgumentNode` and `ValueNode` removed, `ObjectValueNode` added, all consistent with usage.
- Duplication: the deleted `stringifyValue` was a copy of the helper at `src/utilities/findBreakingChanges.ts:538`; removing the copy is a small improvement.
- New branching, flags, optional parameters, casts: none added.
- Existing tests on `review-head`: `BABEL_DISABLE_CACHE=1 ./node_modules/.bin/mocha src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts src/utilities/__tests__/sortValueNode-test.ts`, 48 passing.
- Clone integrity: `git status --short` empty and `HEAD` at `efdbbcfacd92adb78e002f2b8a61f2a6a1504c19` after all runs; all scratch trees live under the work directory.
