# 01 — GraphQLError: the stack-fallback test and the split constructor signature

Covers summary findings 1 and 2. Files: `src/error/GraphQLError.js`, `src/error/__tests__/GraphQLError-test.js`. All line numbers are at the head, `7e39a122eea9292eeffa6905ffdf8a60c5161cfd`.

## Measurements

File sizes at head (`wc -l`): `src/error/GraphQLError.js` 220, `src/error/__tests__/GraphQLError-test.js` 150. Neither is near the 1k-line boundary. Diffstat for the two files: `GraphQLError.js` +1 −1, `GraphQLError-test.js` +27 −33.

Test run at head, from the clone root:

```
./node_modules/.bin/mocha --require @babel/register --require @babel/polyfill \
  src/error/__tests__/GraphQLError-test.js src/error/__tests__/locatedError-test.js \
  src/error/__tests__/printError-test.js src/jsutils/__tests__/inspect-test.js
```

Result: 27 passing (13 of them in `GraphQLError`).

## Finding 1 — the "no stack" test no longer has a stackless original error

### Evidence

The stack logic under test, `src/error/GraphQLError.js` lines 194–209:

```js
  // Include (non-enumerable) stack trace.
  if (originalError && originalError.stack) {
    Object.defineProperty(this, 'stack', {
      value: originalError.stack,
      ...
    });
  } else if (Error.captureStackTrace) {
    Error.captureStackTrace(this, GraphQLError);
  } else {
    Object.defineProperty(this, 'stack', { value: Error().stack, ... });
  }
```

The test at the merge-base (`git show main:src/error/__tests__/GraphQLError-test.js`, lines 44–52):

```js
  it('creates new stack if original error has no stack', () => {
    const original = { message: 'original' };
    const e = new GraphQLError('msg', null, null, null, null, original);
    expect(e.name).to.equal('GraphQLError');
    expect(e.stack).to.be.a('string');
    ...
```

The test at the head (lines 57–64):

```js
  it('creates new stack if original error has no stack', () => {
    const original = new Error('original');
    const e = new GraphQLError('msg', null, null, null, null, original);
    expect(e.name).to.equal('GraphQLError');
    expect(e.stack).to.be.a('string');
    ...
```

And its neighbour, unchanged, at lines 41–55, which also starts with `const original = new Error('original');` and asserts `expect(e.stack).to.equal(original.stack)`.

After the PR the two tests build the same fixture and exercise the same branch (line 195, true). The second one differs only in passing `null` instead of `undefined` for the unused arguments and in asserting less.

### Verification status: confirmed by execution

A scratch mocha file under the work directory (`scratch/stack-probe-test.js`, outside the clone, importing the clone's `src/error/GraphQLError` read-only) checked three things, all passing:

1. Head fixture: `new Error('original')` has `typeof original.stack === 'string'`, and the resulting `e.stack === original.stack`. So the head test goes through the "reuse the original stack" branch, not the "create a new stack" branch.
2. Base fixture: `{ message: 'original' }` has `original.stack === undefined`, the resulting `e.stack` is a string and is not `original.stack`. So the base test did go through the fallback.
3. Proposed fixture (below): `new Error('original')` followed by `delete original.stack` leaves `original.stack === undefined`, and the resulting `e.stack` is a fresh string.

Line coverage does not reveal the regression, because the `Error.captureStackTrace` branch is still reached by every test that passes no original error at all (for example "has a name, message, and stack trace"). What is lost is the specific combination the title describes: an original error is present, and it has no stack. A mutation that changes line 195 to `if (originalError)` would set `stack` to `undefined` in that case; the base test's `to.be.a('string')` would have caught it and the head test cannot.

### Why this matters

The change was made to satisfy `originalError?: ?Error`, which a plain object does not satisfy. That is a legitimate constraint, but the resolution chosen keeps the test's name and discards its subject. A reader now sees two tests that claim to cover both sides of a branch while only one side is covered, which is worse than having one honest test.

### Worked proposal

Keep the fixture a real, correctly typed `Error` and remove the one property the test is about:

```js
  it('creates new stack if original error has no stack', () => {
    const original = new Error('original');
    delete original.stack;
    expect(original.stack).to.equal(undefined);

    const e = new GraphQLError('msg', null, null, null, null, original);
    expect(e.name).to.equal('GraphQLError');
    expect(e.stack).to.be.a('string');
    expect(e.message).to.equal('msg');
    expect(e.originalError).to.equal(original);
  });
```

The precondition assertion is the important addition: it makes the test fail loudly if a future fixture change again hands it an error that does have a stack.

Runtime behaviour of this form is verified (probe 3 above). Whether Flow 0.86 accepts `delete` on the non-optional `stack` property was not verified, because `flow check` was outside this review's execution allowance. If it does not, the fallback is a cast confined to that one statement, `delete (original: any).stack;`, which still leaves `original` typed as `Error` at the constructor call.

## Finding 2 — the constructor signature exists twice and only one copy was widened

### Evidence

`src/error/GraphQLError.js`, declaration (lines 22–31), as changed by the PR:

```js
declare class GraphQLError extends Error {
  constructor(
    message: string,
    nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void | null,
    source?: ?Source,
    positions?: ?$ReadOnlyArray<number>,
    path?: ?$ReadOnlyArray<string | number>,
    originalError?: ?Error,
    extensions?: ?{ [key: string]: mixed },
  ): void;
```

Implementation (lines 92–100), untouched by the PR:

```js
export function GraphQLError( // eslint-disable-line no-redeclare
  message: string,
  nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void,
  source?: ?Source,
  positions?: ?$ReadOnlyArray<number>,
  path?: ?$ReadOnlyArray<string | number>,
  originalError?: ?Error & { +extensions: mixed },
  extensions?: ?{ [key: string]: mixed },
) {
```

The PR's one-line production change is `- nodes?: … | void,` → `+ nodes?: … | void | null,` at line 25 only.

The body that consumes `nodes` (lines 102–108):

```js
  const _nodes = Array.isArray(nodes)
    ? nodes.length !== 0
      ? nodes
      : undefined
    : nodes
    ? [nodes]
    : undefined;
```

### Verification status

Confirmed by reading: the two signatures differ on `nodes` after the PR. Confirmed by execution: `new GraphQLError('msg', null)` yields `e.nodes === undefined` and `e.locations === undefined` (scratch probe), so there is no runtime defect today. Not verified: what `flow check` reports for either spelling, since Flow was not run.

### Why this matters

The two-copy arrangement is pre-existing and has a reason (a function-style constructor needs a `declare class` to give callers a class type). The cost of that arrangement is that the copies must be edited together, and this PR is the case where they were not. After it, the body is checked against "`nodes` is never `null`" while callers are told "`nodes` may be `null`". The current body tolerates the difference by accident of using truthiness; a later edit that, say, replaces the truthiness test with `nodes === undefined` would type-check cleanly against line 94 and break callers that line 25 invites.

There is also a consistency cost. Within these two signatures, nullable-and-optional is written `?T` five times per copy. `nodes` was already the exception (`| void`), and the PR extends the exception (`| void | null`) instead of removing it.

Note that a second, pre-existing divergence sits on `originalError` (`?Error` at line 29 against `?Error & { +extensions: mixed }` at line 98). It is not introduced by this PR and is not raised as a finding, but it is the same hazard and is a reason not to add another.

### Worked proposal

Edit both lines in the same commit, with the spelling the siblings use:

```js
// line 25 and line 94
    nodes?: ?($ReadOnlyArray<ASTNode> | ASTNode),
```

If Flow 0.86 treats the parenthesised maybe-union differently from the explicit union, use the explicit union on both lines instead:

```js
    nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void | null,
```

Either way the rule is the same: one contract, stated identically in both places.

### Alternative depending on the author's intent

If `null` is not meant to be part of the public contract (see the question in the summary), the cleaner change is to revert line 25 and pass `undefined` from the tests, as the "uses the stack of an original error" test in the same file already does. No non-test code in `src/` passes a literal `null` as `nodes`; the only non-literal case is `src/error/locatedError.js` line 31, which passes an `any`-typed expression.

## Observations not raised as findings

The module-level fixture in `GraphQLError-test.js` (lines 17–26) is a net improvement: the base had the same `new Source` / `parse` / dig sequence written out in four tests and a fifth inline variant, and the head has it once. The updated expectations are consistent with the dedented text `"{\n  field\n}\n"`: `field` starts at offset 4 (line 2, column 3) and offset 6 is line 2, column 5.

The single-argument `invariant(...)` calls at lines 24 and 26 throw `new Error(undefined)`, whose message is the empty string (confirmed by execution). At module scope that would surface as an unlabelled load failure for the whole file. This idiom predates the PR in the test suite (`src/utilities/__tests__/getOperationRootType-test.js` line 45 is the same narrowing line), so it is noted here and not charged to this change.
