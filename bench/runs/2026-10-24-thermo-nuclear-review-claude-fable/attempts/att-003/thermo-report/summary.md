# Thermo Nuclear Review — graphql/graphql-js#1582

**Range:** `5384d218539dbb6bb39b25e0b7a5dcdd69ad8a11..7e39a122eea9292eeffa6905ffdf8a60c5161cfd` (`main...review-head`)
**Title:** Enable Flow typings on errors tests + Fix typing for Error constructor
**Scope of change:** one production type-signature line in `src/error/GraphQLError.js`; four test files flipped from `@noflow` to `@flow strict` with the edits needed to type-check.

## What I examined

- The full diff and the head versions of all five changed files.
- `src/error/GraphQLError.js` end to end (the `declare class` public type and the runtime `function GraphQLError` implementation).
- `src/jsutils/dedent.js` and `src/jsutils/invariant.js`, to recompute the new expected offsets and to confirm the refinement helper.
- `src/language/kinds.js`, `src/language/index.js`, `src/index.js` to confirm `Kind` is exported from both import sites used by the tests.
- `.flowconfig` (Flow `^0.86.0`; no `[lints]` or `[strict]` section, so `@flow strict` enables no extra lints and `any` annotations are permitted).
- An existing `@flow strict` test (`src/utilities/__tests__/getOperationRootType-test.js`) that already uses the identical `invariant(node && node.kind === Kind.X)` refinement pattern, confirming it type-checks under the repo's Flow version.
- Ran the four changed test files under the clone's mocha binary: 27 passing, 0 failing. The clone tree was verified clean afterwards.

Per the execution allowance I ran mocha only; `flow check` was not executed. Flow correctness below is therefore established by manual tracing against the repo's existing typed code, not by running the checker.

## Production change: `src/error/GraphQLError.js:25`

The only non-test change widens the `nodes` parameter of the `declare class GraphQLError` constructor from `$ReadOnlyArray<ASTNode> | ASTNode | void` to `... | void | null`. The runtime implementation already handles `null` (`Array.isArray(null)` is false, then `nodes ? [nodes] : undefined` yields `undefined`), and the library itself passes `null` here in several places that previously only compiled because the call sites were untyped. This is a pure widening of an accepted input type: no existing caller can break, no runtime behaviour changes, and no public exported type narrows. The runtime function's own annotation on line 94 still omits `null`; Flow does not unify the `declare class` with the same-named function in this repo's long-standing pattern, so this is a cosmetic inconsistency and not an error source. No security, breaking-change, devex, or feature-gate concern.

## Test changes

- **`GraphQLError-test.js`**: the fixture document is hoisted to module scope and dedented, so expected offsets change. I recomputed them from `dedent` output `"{\n  field\n}\n"`: `field` starts at offset 4 (line 2, column 3); offset 6 is line 2, column 5; the JSON serialization expects line 2, column 3. All match the new assertions and mocha confirms. `new GraphQLError()` became `new GraphQLError('str')` to satisfy the required `message: string`; the `instanceof` assertions are unaffected.
- **`printError-test.js`**: node extraction is rewritten with `invariant` refinements on `kind` and `fields`; the passed nodes (`fieldA.type`, `fieldB.type`) are the same `TypeNode`s as before and the expected printed output is unchanged.
- **`locatedError-test.js`**: two `Error` objects that get ad-hoc properties attached are annotated `any`. No behavioural change.
- **`inspect-test.js`**: a `$FlowFixMe` suppression is added before the `String.raw` tagged-template line. If the suppression turns out to be unnecessary Flow reports only an unused-suppression warning, which does not fail `flow check`.

## Findings

### [Low] `creates new stack if original error has no stack` no longer exercises the stackless-original path

**File:** `src/error/__tests__/GraphQLError-test.js:57-64` (diff hunk replacing `{ message: 'original' }` with `new Error('original')` at line 58).

**Evidence:** Before this change the test passed a plain object `{ message: 'original' }` as `originalError`. A plain object has no `stack` property (verified: `({message:'original'}).stack === undefined`), so `GraphQLError.js:195` (`originalError && originalError.stack`) was false and the constructor fell through to the `Error.captureStackTrace` branch at `GraphQLError.js:201`, which is the behaviour the test title describes. The change substitutes `new Error('original')`, whose `.stack` is always a string in V8 (verified). With that input line 195 is true and the error simply adopts `original.stack`, which is exactly what the preceding test `uses the stack of an original error` (lines 41-55) already asserts. The two tests are now equivalent except for `null` versus `undefined` placeholders; the assertions still pass, so mocha cannot detect the loss.

**Failure it causes:** The code path "an `originalError` was supplied but carries no `stack`" is no longer covered by any test. A future regression on `GraphQLError.js:195-209` (for example, a refactor that unconditionally copies `originalError.stack`, leaving `e.stack` undefined when the original is stackless) would not be caught, even though a test whose name promises precisely that guarantee continues to pass. This is a silent test-coverage regression introduced by the typing migration, not a runtime defect. A typed alternative that preserves the original intent is `const original: any = { message: 'original' };` or deleting `stack` from a real `Error` before passing it.

## Conclusion

No High or Medium findings. The production change is a safe type widening with no runtime effect, and the test migrations preserve the behaviour under test except for the one Low coverage regression above. Nothing in this PR touches secrets, environment, ports, scripts, build steps, or any feature gate.
