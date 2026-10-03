# Error constructor contract and fixtures

## Scope and measurements

Reviewed the complete committed diff, both versions of `GraphQLError-test.js`, the complete head constructor implementation, and the canonical AST, `Source`, location, `dedent`, and `invariant` relationships used by these tests. Guidance files were not loaded as instructions. No external review record informed the finding.

| File | Base lines | Head lines | Change |
| --- | ---: | ---: | --- |
| `src/error/GraphQLError.js` | 220 | 220 | Declared constructor admits null nodes |
| `src/error/__tests__/GraphQLError-test.js` | 156 | 150 | Strict Flow, shared AST fixture, typed original error |

Measurements came from `git diff --numstat main...review-head` and a read-only Python script counting `splitlines()` in `git show main:<path>` and the corresponding head file. The constructor adds no statements, branches, helpers, state, or asynchronous work. The test introduces two canonical invariants and no `any` annotation or suppression. It removes six net lines. Neither file crosses the skill's 1,000-line boundary.

The base test parsed its fixture separately in five cases; the head parses one shared document. Its AST objects are only inspected in these tests. `GraphQLError` wraps a supplied node or reads an existing array, derives positions and locations, and defines its own error properties. It does not mutate the shared AST or `Source`. Sharing the fixture therefore does not create an observed order dependency.

## Constructor contract assessment

At `src/error/GraphQLError.js:25`, the declared class constructor changes `nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void` to the same union plus `null`. The runtime implementation at lines 102–108 already maps a null input to `undefined` nodes. The other optional constructor arguments already admit nullable inputs. Existing tests supply null placeholders to reach later arguments, making this a concrete compatibility correction rather than gratuitous optionality.

The implementation function's own node annotation at line 94 remains unchanged. That annotation describes an unchanged implementation, and the declared class constructor is the consumer-facing construction contract edited here. There is no runtime difference to attribute to this discrepancy, and the review did not execute Flow to establish a static-checking failure. It is not a separate finding.

The simplest architecture is the existing normalization in the constructor. Adding a null adapter or nullable-node helper would duplicate canonical behavior. Rewriting the constructor's nested node-normalization expression or its descriptor table would enlarge this typing PR without addressing a demonstrated regression.

The dedented shared source is `"{\n  field\n}\n"`. Its field begins at offset 4, line 2, column 3; offset 6 corresponds to line 2, column 5. The operation starts at offset 0, line 1, column 1. Those values match the changed assertions at lines 70–71, 78–79, 86–87, and 94–95. Reusing the field for serialization moves the expected location from line 1 to line 2 consistently.

The `Kind.OPERATION_DEFINITION` check at test line 24 refines the discriminated `DefinitionNode` union before accessing `selectionSet`. The field-existence check at line 26 expresses the fixture's array-element assumption. `src/utilities/__tests__/getOperationRootType-test.js:43–46` already uses this operation-node refinement pattern. The PR reuses `invariant`, rather than introducing a bespoke AST casting helper.

## F1 — Preserve the stackless-original-error fixture

The changed line is `src/error/__tests__/GraphQLError-test.js:58`, inside the case spanning lines 57–64. The base and head fixtures differ in the one property that controls the intended branch:

```js
// Base: no stack property.
const original = { message: 'original' };

// Head: a native Error with an existing stack in the permitted Node runner.
const original = new Error('original');
```

Both versions wrap the fixture with `new GraphQLError('msg', null, null, null, null, original)`. The four assertions check its name, a string stack, message, and original-error identity. None asserts that the input stack is missing or that a fresh stack was created.

The constructor chooses the copying branch when `originalError && originalError.stack` is truthy at lines 195–200. Otherwise it captures a new stack at lines 201–202, or uses the non-capture fallback at lines 203–208. The new fixture exercises the copying branch, as does the preceding “uses the stack of an original error” case at test lines 41–55. The “has a name, message, and stack trace” case uses no original error and cannot replace coverage of an original error with a missing stack.

The regression is specific: a future constructor change that copies `originalError.stack` whenever an original error exists can silently leave the wrapper's stack undefined. The old fixture detects this; the head fixture no longer does. The runtime implementation is unchanged and currently generates a stack correctly. This is P2 test-coverage loss introduced by the typing migration.

## Verification: an isolated mutation distinguishes the fixtures

The scratch harness is at `../stack-regression-test.js`, outside the clone. It reads the head constructor and compiles a separate module in memory with precisely this replacement:

```js
// Correct guard.
if (originalError && originalError.stack) {

// Mutant guard.
if (originalError) {
```

The rest of the constructor remains identical. The mutant uses the clone's Babel configuration and module resolution; its private module object does not replace the canonical require cache entry. The harness implements all four head-test assertions with Node's strict assertions.

| Experiment | Observed result |
| --- | --- |
| Head fixture with real constructor | Has an existing stack; wrapper copies exactly that stack |
| Head fixture with mutant | All four current assertions pass |
| Base stackless fixture with real constructor | Generates a string stack; all four assertions pass |
| Base stackless fixture with mutant | Wrapper stack is undefined; string-stack assertion fails |
| Typed Error with stack explicitly undefined, real constructor | Generates a string stack; all four assertions pass |
| Same repaired fixture with mutant | Wrapper stack is undefined; string-stack assertion fails |
| Null nodes versus undefined nodes, real constructor | Same absent nodes, source, positions, locations, and serialized result |

The harness asserts that the failures occur as expected, so its **5 passing** result confirms the regression experiment rather than reporting the mutant as a correct implementation.

The experiment proves that the particular head test lost its input-state distinction. It does not claim that every possible stack-generation regression escapes every test in the repository, or that the non-V8 stack fallback was separately exercised.

## Worked code-judo repair

Keep the public boundary typed as `Error` and make the exceptional runtime state explicit in the existing case. This avoids replacing a precise contract with an error-like union solely to accommodate a test, and avoids a new fixture factory for one input:

```js
it('creates new stack if original error has no stack', () => {
  const original = new Error('original');
  Object.defineProperty(original, 'stack', { value: undefined });
  expect(original.stack).to.equal(undefined);

  const e = new GraphQLError('msg', null, null, null, null, original);
  expect(e.name).to.equal('GraphQLError');
  expect(e.stack).to.be.a('string');
  expect(e.message).to.equal('msg');
  expect(e.originalError).to.equal(original);
});
```

The property API preserves the fixture's `Error` type without `any`. The harness verifies the repair's runtime effect and proves that it rejects the mutant. Its exact Flow compatibility was not executed; the project's checker remains the appropriate static verification before accepting a remedy. No repair was applied to the checkout.

There is no valuable larger abstraction to extract here: the preceding stack-copy test and this stack-generation test should remain separate, explicit states. Parameterizing them behind a new helper would obscure the branch distinction that this change accidentally erased.

## Commands and verification status

The target test command, run once from the clone, was:

```sh
BABEL_DISABLE_CACHE=1 ./node_modules/.bin/mocha --require @babel/register --require @babel/polyfill src/error/__tests__/GraphQLError-test.js src/error/__tests__/locatedError-test.js src/error/__tests__/printError-test.js src/jsutils/__tests__/inspect-test.js
```

Result: exit 0, **27 passing**, roughly two seconds wall time. This includes all 13 constructor cases. It establishes current runtime behavior, not successful Flow checking.

The scratch experiment, run once from the clone, was:

```sh
BABEL_DISABLE_CACHE=1 ./node_modules/.bin/mocha --require @babel/register --require @babel/polyfill /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-003/clone-work/stack-regression-test.js
```

Result: exit 0, **5 passing**, under one second wall time. Both commands emitted the existing outdated-Browserslist warning. No network, installation, coverage output, checkout edits, or Babel cache writes were used.

The finding is verified by source comparison and the mutation experiment. Full-suite behavior and Flow 0.86.0 checking were not executed under the focused-test allowance.
