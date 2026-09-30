# Thermo-nuclear code quality review — graphql/graphql-js#1582

Range: `5384d218..7e39a122` ("Enable Flow typings on errors tests + Fix typing for Error constructor"), 5 files, +48/−47.

## Verdict

Changes requested, though the list is short. The direction is right. Moving four test files from `@noflow` to `@flow strict` is useful, and so is collapsing the repeated ad-hoc sources in `GraphQLError-test.js` into one shared `dedent`ed fixture. That fixture is a real simplification. But the conversion has a cost. In one place a test was made to type-check by changing what it tests. The one production-code typing fix was applied to only one of the two signatures that describe the same constructor. And one test file now repeats a longer, misnamed, over-guarded extraction block. None of this is large, and all of it is cheap to fix before merging. There are no file-size, layering or orchestration concerns. Every touched file is under 225 lines, and the only non-test change is one line.

All 27 tests in the four touched files pass at the head (`mocha --require @babel/register --require @babel/polyfill`). Flow was not executed in this run because the allowance covers focused mocha runs only. Type-level claims below are reasoned from Flow semantics and labeled that way.

## Findings

### 1. A stack-trace test was rewritten into a duplicate of its neighbor (test-coverage regression)

In `src/error/__tests__/GraphQLError-test.js:57-64`, the test "creates new stack if original error has no stack" used to pass `{ message: 'original' }` as the original error. To satisfy Flow's `?Error` parameter, the PR changed that to `new Error('original')`. A real V8 `Error` always has a `stack`, so the constructor now takes the `originalError.stack` branch at `src/error/GraphQLError.js:195-200`. That is the same branch the preceding test, "uses the stack of an original error", already covers. The fallback this test is named for is no longer exercised with an original error present. The assertion only checks that `e.stack` is a string, so it cannot notice. I checked this with a scratch script against the real module: with the PR's fixture, `e.stack === original.stack` is `true`. The fix is to keep a real `Error` but make it stackless (`delete original.stack`, which I also checked at runtime), and to assert `e.stack !== original.stack`. If Flow rejects the `delete`, fall back to a local `const original: any = { message: 'original' }`, the same idiom the PR already uses in `locatedError-test.js`. Full evidence and the worked test are in `02_error-and-jsutils-tests.md` (Finding 2.1).

### 2. The `nodes` widening was applied to the declared contract but not the implementation, in an inconsistent spelling

In `src/error/GraphQLError.js:25`, the PR widens `nodes` in the `declare class` constructor to `$ReadOnlyArray<ASTNode> | ASTNode | void | null`. The widening is correct, and the runtime already treats `null` like `undefined`. But the actual `export function GraphQLError` signature at line 94 still says `… | void`. The file now states two different contracts for one parameter, and the declare/implementation pair already drifted on `originalError`, so this adds a second drift. The longhand `| void | null` also sits among siblings that all use Flow's `?T` shorthand. The author's stated goal was to make `nodes` consistent with the other arguments, so the consistent spelling is `nodes?: ?($ReadOnlyArray<ASTNode> | ASTNode)`, and it should be written in both places. This is behavior-preserving and removes drift instead of adding it. Worked proposal in `01_error-constructor-typing.md` (Finding 1.1).

### 3. `printError-test` copy-pastes a misnamed extraction block and adds no-op guards

In `src/error/__tests__/printError-test.js:62-80`, the Flow conversion turns a one-liner per document into a three-line extract, refine and index sequence written twice. It names the results `opA`/`opB`, although the next line's `invariant` asserts they are object type definitions, not operations. It then adds a separate `invariant(fieldA && fieldB)` on line 80. Flow types array index reads as `T`, not `?T`, so that `invariant` and the `opA &&`/`opB &&` conjuncts refine nothing. The same no-op guards appear in the new shared fixture at `src/error/__tests__/GraphQLError-test.js:24-26` (`operationNode &&` and the whole `invariant(fieldNode)`). A small local helper, `firstFieldType(doc)`, would do the needed refinements once: the `kind` check, and the `fields` check because `fields` is optional in the AST type. That deletes the duplicated block and the dead guards and restores the original `fieldTypeA`/`fieldTypeB` names. This is a reasoned claim: Flow was not run. Worked code in `02_error-and-jsutils-tests.md` (Finding 2.2).

### 4. (Minor) A bare `$FlowFixMe` suppresses a whole assertion line to keep a four-character `String.raw`

In `src/jsutils/__tests__/inspect-test.js:31-32`, the PR adds an unexplained `// $FlowFixMe` only because Flow's lib def rejects `String.raw` as a template tag. `String.raw` is there only to avoid one backslash escape. Writing `expect(inspect('"')).to.equal('"\\""');` removes the tag and the suppression. It also stops the suppression from hiding any future type error in the `inspect` call on that line. There is precedent in `schemaPrinter-test.js:178`, which is why this is minor. If the suppression stays, it should give a reason, as `objectValues.js` and `isInteger.js` do. Details in `02_error-and-jsutils-tests.md` (Finding 2.3).

## What is good and should stay

The shared module-level fixture in `GraphQLError-test.js` removes four copies of an ad-hoc indented source, and the expected positions and columns were updated consistently. Using `const e: any` in `locatedError-test.js` is an honest, local way to say "this is deliberately not a real GraphQLError". Passing a message to `new GraphQLError('str')` is correct.

## Proposed remediation sequence

1. Restore the stackless-original scenario in `GraphQLError-test.js:57-64` and tighten its assertion (Finding 1). This is the only item that changes what the suite verifies.
2. Align both `nodes` signatures in `GraphQLError.js` (lines 25 and 94) on `?($ReadOnlyArray<ASTNode> | ASTNode)` (Finding 2).
3. Collapse the duplicated extraction in `printError-test.js` into a `firstFieldType` helper and delete the no-op `invariant`s there and in `GraphQLError-test.js:24-26` (Finding 3).
4. Replace the `String.raw` + `$FlowFixMe` pair in `inspect-test.js` with a plain literal (Finding 4).

## Detail files

- `01_error-constructor-typing.md` covers the `declare class` vs. implementation signature in `src/error/GraphQLError.js`.
- `02_error-and-jsutils-tests.md` covers the four test-file conversions, the commands run, the scratch-script output, and the items considered but not raised.
