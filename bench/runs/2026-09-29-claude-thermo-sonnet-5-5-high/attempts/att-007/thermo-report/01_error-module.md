# 01 — src/error module and its tests (detail)

Scope: `git diff main...review-head` (5 files, +48/−47, one commit). Measurements: no file crosses 1k lines (GraphQLError-test.js shrinks; GraphQLError.js is ~230 lines). Commands used: `git diff main...review-head`, `sed`/`grep` on `src/error/GraphQLError.js` and the test files, and `./node_modules/.bin/mocha --require @babel/register --require @babel/polyfill src/error/__tests__/*.js src/jsutils/__tests__/inspect-test.js` (27 passing). Flow itself was not run (no way to confirm type-check results offline), so Flow-related statements below are by reading.

## Finding 1 — The `declare class` constructor and the real function signature now disagree (verified by reading)

File: `src/error/GraphQLError.js`, lines 25 and 94.

The diff widens `nodes` in the `declare class GraphQLError` constructor (line 25) to `$ReadOnlyArray<ASTNode> | ASTNode | void | null`. The actual implementation signature 70 lines below (line 94, `export function GraphQLError(...)`) still reads `nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void`. The two declarations describe the same contract and are now out of step. Runtime already tolerates `null` (the body uses `Array.isArray(nodes)` / truthiness), so the implementation signature is simply stale. Other parameters on both signatures use `?T` for nullability, and the ad-hoc `| void | null` union on `nodes` is the one place the idiom is different. A second, pre-existing drift: `originalError` is `?Error & { +extensions: mixed }` in the implementation but `?Error` in the declaration.

Code-judo proposal: the pattern of a `declare class` plus a hand-copied function signature means every contract change has to be made twice. Make the contract change in one place by writing `nodes?: ?($ReadOnlyArray<ASTNode> | ASTNode)` in both (this also matches the `?Source`, `?$ReadOnlyArray<number>` style of the neighbours), and long-term derive the implementation signature from the declaration or drop the duplicate types so only one copy can drift.

Remediation: update line 94 to match line 25 in the same commit, using the `?(...)` idiom.

## Finding 2 — Test 'creates new stack if original error has no stack' no longer tests the no-stack branch

File: `src/error/__tests__/GraphQLError-test.js`, line 57.

To satisfy Flow, the fixture changed from `{ message: 'original' }` (an object with no `stack`) to `new Error('original')`. A real `Error` always has a `stack`, so this test now exercises the same `originalError.stack` branch as the preceding test 'uses the stack of an original error'. The `else if (Error.captureStackTrace)` / `Error().stack` fallbacks in `GraphQLError.js` (about lines 202–211) lose their only covering test, while the title still claims otherwise. The assertions (`e.stack` is a string) pass either way, so this is a silent loss of coverage introduced by the typing pass, not a failure.

Remediation: keep the type-clean fixture but strip the stack, for example create `const original = new Error('original'); delete original.stack;` (or set `original.stack = undefined` behind a narrow, commented `$FlowFixMe`), and additionally assert `expect(e.stack).to.not.equal(original.stack)`. Do not let a type-driven fixture change silently change what a test proves.

## Finding 3 — Module-level shared fixtures with import-time `invariant` narrowing, and re-derived magic offsets

File: `src/error/__tests__/GraphQLError-test.js`, lines 14–26 and the assertions at roughly 67–95.

Three tests each parsed their own small document. The change hoists one `source`/`ast`/`operationNode`/`fieldNode` to module scope and narrows with `invariant(...)` at import time. This removes duplication, which is good, but has costs: a failure in the fixture now aborts loading the whole file rather than one test, the narrowing is done by throwing invariants rather than through a helper, and every hard-coded offset (`[4]`, `{line: 2, column: 3}`, `[6]`, column `5`) was recomputed by hand because the document text changed. Those constants are now coupled to the `dedent` layout of a fixture 50 lines away. Similarly, in `printError-test.js` (lines 62–80) the same "find the first field of the first object type" narrowing is written twice (`opA`/`opB`) with a three-part invariant each, and the variable names `opA`/`opB` are misleading because they hold type definitions, not operations.

Code-judo proposal: add one small test helper (for example `getFirstField(doc)` in a shared test util that does the `invariant` narrowing and returns the field node), and use it in both files. That deletes the four invariant lines in printError-test, the module-level import-time narrowing, and makes the intent ("first field") readable. Prefer `source.body.indexOf('field')` for positions instead of hand-written numbers where practical.

## Finding 4 — Blanket `any` and an unexplained `$FlowFixMe` in tests that are being newly opted into `@flow strict`

Files: `src/error/__tests__/locatedError-test.js` lines 29 and 41; `src/jsutils/__tests__/inspect-test.js` line 31.

`const e: any = new Error(...)` is used so the test can attach `locations`, `path` and `nodes`. It turns off checking for the whole variable rather than modelling the "GraphQLError-ish" and "elasticsearch-like" shapes the tests are explicitly about. In `inspect-test.js` a bare `// $FlowFixMe` was added above `inspect('"')` with no explanation; nothing in the file says what Flow objects to (the argument is a valid string, so the suppression looks like a workaround for how the lint/Flow rule treats `String.raw` on the following line). Under `@flow strict` an unexplained suppression tends to be copied forward and never removed.

Remediation: use `Object.assign(new Error('...'), { locations: [], path: [], nodes: [] })` typed as a tiny local shape, or at minimum give the `$FlowFixMe` a one-line reason. These are minor and should not block the change.

## Things checked and found acceptable

- The `GraphQLError.js` change is a one-token type widening with no runtime effect. The constructor already handles `null` for `nodes`; the motivation (all other params accept `null`, so `nodes` should too) is sound. The `| void | null` spelling is the only nit (Finding 1).
- `new GraphQLError()` → `new GraphQLError('str')` in the subclass test is a correct fix; `message` is a required argument.
- Import reordering in `printError-test.js` (jsutils first) is cosmetic and consistent with the rest of the repo.
- No size-threshold, branching, or layering concerns: the diff adds no production logic.
