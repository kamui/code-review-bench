# 01 — `src/error`: GraphQLError contract and error tests

Scope: `src/error/GraphQLError.js`, `src/error/__tests__/GraphQLError-test.js`,
`src/error/__tests__/locatedError-test.js`, `src/error/__tests__/printError-test.js`.
Range: `5384d218..7e39a122` (`git diff main...review-head`).

## Measurements and commands

- `git diff main...review-head` — 5 files, +48/−47. No file gets anywhere near 1000 lines
  (`GraphQLError.js` is ~230 lines; the test files are all under 170). There is no file-size concern.
- `./node_modules/.bin/mocha --require @babel/register --require @babel/polyfill` over the four
  changed test files: **27 passing**.
- Scratch probe `clone-work/scratch/stack-probe.js` (run from the clone root with `@babel/register`)
  builds `new GraphQLError('msg', null, null, null, null, new Error('original'))` and prints:
  `new Error has stack: true`,
  `GraphQLError reused original.stack (captureStackTrace branch skipped): true`,
  `plain object: fresh stack generated: true`.
- Scratch probe `clone-work/scratch/stack-probe2.js` sets `original.stack = ''` on a real `Error`
  and prints `empty-stack original -> fresh stack: true`.
- Flow itself was **not** run. The execution allowance covers only focused mocha runs, so every
  statement below about what Flow accepts comes from reading the code and `.flowconfig`
  (Flow `^0.86.0`). None of it is confirmed by a type-check.

---

## Finding 1.1 — The "no stack" test was rewritten into a duplicate of the "has stack" test, so the fallback branch is no longer covered

**Where:** `src/error/__tests__/GraphQLError-test.js:57-64` (head), compared with `:41-55`.
The branch under test is `src/error/GraphQLError.js`, the `if (originalError && originalError.stack)
… else if (Error.captureStackTrace) … else …` block near the end of the constructor.

**Evidence.** Before this PR the test `creates new stack if original error has no stack` passed
`{ message: 'original' }`. That object has no `stack` property, so the constructor skipped the
`originalError.stack` branch and built a fresh stack. To satisfy `@flow strict`, the PR changed the
fixture to `new Error('original')`. A real `Error` always has a `stack` string in V8. The constructor
now takes the first branch and copies `original.stack`, which is exactly what the preceding test
`uses the stack of an original error` already checks. The scratch probe confirms this:
`e.stack === original.stack` is `true` for the new fixture. The test's assertions
(`e.stack` is a string, name, message, originalError) are weak enough that they still pass, so
mocha reports 27 green tests while the test no longer does what its name says.

**Why it matters.** This is a regression in test quality hidden inside a typing change. The
`captureStackTrace` fallback is now covered only by accident, through tests that pass no
`originalError`. The "original error without a stack" case, which is the one the test name
promises, has no coverage at all. If someone later changed the first condition to
`if (originalError)`, this test would keep passing even though a stackless original would then
produce `stack: undefined`.

**Verification status:** Confirmed by executing the constructor (probe output above).

**Remedy.** Keep the typed fixture but actually remove the stack, and make the test check the
branch it names:

```js
it('creates new stack if original error has no stack', () => {
  const original = new Error('original');
  original.stack = ''; // falsy: forces the fresh-stack branch, still a well-typed Error
  const e = new GraphQLError('msg', null, null, null, null, original);
  expect(e.name).to.equal('GraphQLError');
  expect(e.stack).to.be.a('string').and.not.equal(original.stack);
  expect(e.message).to.equal('msg');
  expect(e.originalError).to.equal(original);
});
```

`stack-probe2.js` confirms that an empty `stack` sends execution down the fresh-stack path. If
the author wants to test the truly off-contract "stack is missing" case, the honest alternative is
`const original = ({ message: 'original' }: any);`. It is explicit about leaving the type contract,
which is better than silently changing what is being tested.

---

## Finding 1.2 — `nodes` was widened on only one of the two hand-maintained constructor signatures

**Where:** `src/error/GraphQLError.js:25` (the `declare class GraphQLError` constructor) and
`src/error/GraphQLError.js:94` (the `export function GraphQLError` implementation).

**Evidence.** The PR changes the declared constructor to
`nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void | null`. The implementation signature 70 lines
below still reads `nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void`. The two copies had already
drifted before this PR: the declared `originalError?: ?Error` (line 29) differs from the
implementation's `originalError?: ?Error & { +extensions: mixed }` (line 98). The runtime already
treats `null` like `undefined`, because the `_nodes` computation falls through to `undefined` on
any falsy value, so the widening is behaviorally correct. The author's thread comment ("All other
arguments support `null` …") explains the intent well.

**Why it matters.** This file relies on a `declare class` / `function` pair that must be kept in
sync by hand, and this PR leaves them out of sync a second time. The declared type is what callers
see; the implementation type is what the body is checked against. When the two disagree, readers
cannot tell which one is the contract, and the next person to touch `nodes` has two places to
update and nothing to warn them. The spelling `?` + `| void | null` is also a roundabout way to write
what the neighbouring parameters already say with Flow's maybe type.

**Verification status:** The mismatch is confirmed by reading the source. That Flow tolerates the
mismatch today is inferred, not confirmed by running Flow.

**Remedy (code-judo).** Make the two signatures identical and write `nodes` in the same style as
its siblings, so the parameter list reads uniformly:

```js
// both at line 25 and line 94
nodes?: ?($ReadOnlyArray<ASTNode> | ASTNode),
```

The cleaner follow-up is to stop maintaining two copies at all. For example, define one exported
`type GraphQLErrorArgs` tuple, or put a single comment-anchored "keep in sync with the declare
class above" note on both lists and align `originalError` at the same time. The minimum ask for this
PR is that the parameter it explicitly set out to fix is correct in both places.

---

## Finding 1.3 — The Flow narrowing added to the tests is duplicated, partly redundant, and misnamed

**Where:** `src/error/__tests__/printError-test.js:62-80` and
`src/error/__tests__/GraphQLError-test.js:17-26`.

**Evidence.** In `printError-test.js` the same three-line narrowing block appears twice
(`docA`/`opA`/`fieldA`, then `docB`/`opB`/`fieldB`). The variables are named `opA`/`opB`
("operation") even though the invariant checks `Kind.OBJECT_TYPE_DEFINITION`, so they are type
definitions, not operations. The trailing `invariant(fieldA && fieldB)` guards the result of an
array index, and Flow types an index read like `opA.fields[0]` as non-optional, so this line adds
noise without narrowing anything. The same applies to `invariant(fieldNode)` at
`GraphQLError-test.js:26`, and to the `operationNode &&` half of line 24, since `kind === …` already
narrows the union. In `GraphQLError-test.js` these invariants now run at module scope. If the fixture
ever stops parsing into the expected shape, mocha fails while loading the file and reports no
individual failing test.

The shared-fixture hoisting in `GraphQLError-test.js` is a real improvement: three copies of the
same inline `Source` are collapsed into one `dedent`ed source. The position and location updates
(`8→4`, `(2,7)→(2,3)`, `10→6`, `(2,9)→(2,5)`, `1:3→2:3`) are consistent with the dedented text
and pass.

**Verification status:** The duplication and naming are confirmed by reading the code. That the
extra invariants are redundant is inferred from Flow's array-index typing and was not confirmed
by running Flow.

**Remedy.** Collapse the duplicated narrowing in `printError-test.js` into one local helper, name
it for what it returns, and drop the redundant guards:

```js
function firstFieldType(doc) {
  const typeDef = doc.definitions[0];
  invariant(typeDef.kind === Kind.OBJECT_TYPE_DEFINITION && typeDef.fields);
  return typeDef.fields[0].type;
}

const error = new GraphQLError('Example error with two nodes', [
  firstFieldType(parse(new Source(dedent`…`, 'SourceA'))),
  firstFieldType(parse(new Source(dedent`…`, 'SourceB'))),
]);
```

In `GraphQLError-test.js`, reduce the guard to
`invariant(operationNode.kind === Kind.OPERATION_DEFINITION)` and delete `invariant(fieldNode)`.

---

## Non-findings (checked and accepted)

- `locatedError-test.js:29,41` annotates `const e: any = new Error(...)`. These tests exist to feed
  deliberately off-contract "GraphQLError-ish" and "elasticsearch-like" objects into
  `locatedError`, so an explicit `any` is the honest boundary. It does not hide the real contract,
  and `locatedError.js` itself already uses `(originalError: any)` for the same brand-check.
- `new GraphQLError()` → `new GraphQLError('str')` in the subclass test only fixes the
  now-type-checked call. Behavior is unchanged.
