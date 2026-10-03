# Detail: argument comparison in OverlappingFieldsCanBeMergedRule

Subsystem: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, with `src/utilities/sortValueNode.ts` and `src/language/printer.ts` as the helpers the change leans on.

## What the diff does

At the base, `findConflict` read both fields' argument arrays and called `sameArguments(args1, args2)`. That helper returned `false` on a length mismatch, then for each argument of the first field looked up the same-named argument of the second with `find` and compared `print(sortValueNode(value))` of the two values.

At the head, `findConflict` line 591 is:

```ts
if (stringifyArguments(node1) !== stringifyArguments(node2)) {
```

and `stringifyArguments` (lines 637-650) wraps the whole argument list in a synthetic `ObjectValueNode`, sorts it with `sortValueNode`, and prints it:

```ts
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

File size: 838 lines at `main`, 826 at `review-head` (`wc -l`).

## Why the call site is hot

`findConflict` is the body of the nested loops in `collectConflictsWithin` and `collectConflictsBetween`. For a response name with `n` fields in one selection set, `collectConflictsWithin` calls it `n(n-1)/2` times. Each call now runs `stringifyArguments` twice. Per call, that is:

- one array from `args.map`, one object per argument, one wrapper object;
- in `sortValueNode`, a spread copy of the wrapper, a second mapped array with a spread copy per field, a recursive sort of each value, and an `Array.prototype.sort`;
- in `print`, a `visit` traversal over the result with the printer reducer.

None of it depends on the other field in the pair. For a field with no arguments the output is always `{}`.

At the base the same pair cost two `?? []` reads, a length comparison, and `every` over an empty array.

## Measurements

Scratch script `scratch/bench.js` in the work directory, run from copies of each tree made with `git archive main ... | tar -x` and `git archive review-head ... | tar -x`, with `node_modules` symlinked to the clone's. It builds this schema:

```graphql
input In { a: Int, b: String, c: [Int] }
type Query { hello: String, f(x: Int, y: String, z: In): String }
```

and validates with only `OverlappingFieldsCanBeMergedRule`:

- "no-args": `{ hello hello hello ... }` with `n` repetitions.
- "with-args": `{ f(x: 1, y: "abc", z: {c: [1, 2, 3], b: "s", a: 1}) ... }` with `n` repetitions.

Three warm-up validations, then the mean of three timed validations. Command, per tree:

```
BABEL_DISABLE_CACHE=1 timeout 120 node bench.js .
```

Results in milliseconds per `validate` call (Node v24.21.0):

| Case | base `730d5af8` | head `efdbbcfa` | head / base | early return only | early return + memo |
| --- | ---: | ---: | ---: | ---: | ---: |
| no-args, n=250 | 5.48 | 173.13 | 31.6x | 6.67 | 6.00 |
| no-args, n=500 | 17.09 | 710.08 | 41.5x | 21.75 | 20.86 |
| no-args, n=1000 | 60.25 | 2832.39 | 47.0x | 74.06 | 66.49 |
| with-args, n=50 | 123.38 | 179.03 | 1.45x | 180.17 | 4.01 |
| with-args, n=100 | 485.21 | 650.18 | 1.34x | 684.82 | 7.66 |
| with-args, n=200 | 2167.33 | 2670.70 | 1.23x | 2553.16 | 16.12 |

All runs reported zero validation errors, as expected for these documents.

Two things stand out. First, the head makes the argument-less case as slow as the argument-heavy case: both are now dominated by two prints per pair. Second, the base was already quadratic in prints for fields with arguments (2.2 seconds for 200 repeated fields), so the base is not the target to return to. The memoized version is the first one where the argument comparison stops being the dominant cost.

An earlier run of the same script at n=500 with arguments took 13.3 seconds per validation at the base before I stopped it for exceeding the command time limit; I did not obtain the head figure at that size.

## Worked proposal

Keep the single concept the diff introduced, and move its cost from "per pair" to "per field".

```ts
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

The call site at line 591 does not change.

Notes on the shape:

- The empty string for "no arguments" cannot collide with a field that has arguments, because any non-empty argument list prints as at least `{x: ...}`.
- The memo is lazy, so fields whose response name is unique in their selection set (the usual case) are never stringified at all. Computing the string eagerly while building the `NodeAndDef` tuples in `_collectFieldsAndFragmentNames` would be simpler to read but would pay a print for every field with arguments in every document.
- A module-level `WeakMap` is the smallest change. The alternative that matches this file's existing style is a `Map<FieldNode, string>` created in the `SelectionSet` visitor beside `cachedFieldsAndFragmentNames` and `comparedFragmentPairs`, passed down the same call chain. That keeps cache lifetime explicit and scoped to one validation, but adds a parameter to `findConflict` and to the six functions above it that already thread `comparedFragmentPairs` down to it. I would accept either. If the parameter list is the objection, the right follow-up is to bundle the three caches into one context object, which is a separate change.
- AST nodes are treated as immutable throughout the validator, so keying on node identity is safe.

Prototype verification, run from the scratch copies:

```
BABEL_DISABLE_CACHE=1 ./node_modules/.bin/mocha --require resources/ts-register.js --check-leaks 'src/validation/__tests__/*-test.ts'
```

gives `521 passing` for both the early-return-only prototype and the early-return-plus-memo prototype.

## Behavior comparison

Scratch script `scratch/dup.js`, schema `type Query { f(a: Int, b: Int): String }`, only this rule enabled.

| Document | base | head | both prototypes |
| --- | --- | --- | --- |
| `{ f(a: 1, a: 2) f(a: 1, a: 2) }` | differing arguments | valid | valid |
| `{ f(a: 1, a: 2) f(a: 2, a: 1) }` | differing arguments | differing arguments | differing arguments |
| `{ f(a: 1, a: 1) f(a: 1) }` | differing arguments | differing arguments | differing arguments |
| `{ f(a: 1, b: 2) f(b: 2, a: 1) }` | valid | valid | valid |
| `{ f f }` | valid | valid | valid |
| `{ f(a: 1) f }` | differing arguments | differing arguments | differing arguments |

The only difference between base and head is the first row. At the base, `arguments2.find` returned the first argument named `a` for both lookups, so the second `a: 2` was compared against `a: 1` and two identical fields were reported as conflicting. At the head the two fields print identically and are not reported. `UniqueArgumentNamesRule` reports the duplicate independently, so a document like this is still invalid under the standard rule set. No test in `OverlappingFieldsCanBeMergedRule-test.ts` exercises duplicated argument names, at either revision. This is recorded as a question in the summary, not as a finding.

## Test status at the head

```
BABEL_DISABLE_CACHE=1 ./node_modules/.bin/mocha src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts src/utilities/__tests__/sortValueNode-test.ts
```

`48 passing`. The clone's `git status --short` is empty and `HEAD` is `efdbbcfacd92adb78e002f2b8a61f2a6a1504c19` after all runs. `BABEL_DISABLE_CACHE=1` was set so that `@babel/register` wrote nothing under the clone.

## Not verified

- I did not run the repository's own `benchmark/` harness; it builds and compares revisions in ways that are outside the focused-test allowance. The figures above come only from the scratch script.
- I did not measure a mixed real-world query corpus. The synthetic documents isolate the pairwise loop; the ratio on ordinary queries with few repeated response names will be much smaller, since `findConflict` is only reached for fields that share a response name.
- I did not measure the `Map`-threaded variant of the memo, only the `WeakMap` one.
