# Thermo-nuclear code quality review — graphql-js #1582

Range: `5384d218..7e39a122` (one commit, 5 files, +48/−47). Detail evidence lives in `01_error-module.md`.

## Verdict

This is a small, mostly test-only typing pass: it turns `@noflow` into `@flow strict` for three error tests and one jsutils test, and widens the `nodes` parameter of the `GraphQLError` constructor declaration to accept `null`. There is no production logic, no file-size concern, no new branching, and the affected tests pass (27 passing under mocha; Flow was not run). I would not block on structure. There is, however, one real loss of test coverage caused by the typing pass and one contract duplication that the diff makes worse, and both should be fixed before or immediately after merging.

## Findings

The most important finding is in `src/error/__tests__/GraphQLError-test.js` at line 57. The test named 'creates new stack if original error has no stack' used to pass a plain `{ message: 'original' }` object so that `originalError.stack` was absent. To satisfy Flow, the fixture became `new Error('original')`, which always has a stack. The test therefore now walks the same `originalError.stack` branch as the preceding test and no longer covers the `Error.captureStackTrace` / `Error().stack` fallback in `GraphQLError.js`; the assertions still pass, so the coverage loss is silent while the title claims otherwise. The remedy is to keep the well-typed fixture but remove its stack (for example `delete original.stack`, or a narrowly commented `$FlowFixMe`) and to assert the resulting stack is not the original's. A type-driven fixture change should not be allowed to change what a test proves. Details are in `01_error-module.md`, Finding 2.

The second finding is in `src/error/GraphQLError.js` at lines 25 and 94. The diff adds `| void | null` to `nodes` in the `declare class` constructor but leaves the actual `export function GraphQLError(...)` signature 70 lines below unchanged, so the two hand-copied declarations of one contract now disagree (and `originalError` already disagreed between them before this change). Runtime already tolerates `null`, so this is stale typing rather than a bug, but it is exactly the drift a duplicated signature invites. Update both in the same change, use the `?(...)` idiom that the neighbouring parameters use instead of a bespoke `| void | null` union, and consider whether the duplicated types can be collapsed so only one copy exists. See `01_error-module.md`, Finding 1.

The third finding concerns the shape of the new test fixtures in `GraphQLError-test.js` (lines 14–26) and `printError-test.js` (lines 62–80). Hoisting the repeated `parse` calls into module-level constants is an improvement, but the narrowing is done with import-time `invariant` calls (a fixture failure kills the whole file at load), every position and column assertion was recomputed by hand and is now coupled to a `dedent` layout defined far away, and `printError-test.js` writes the same three-clause narrowing twice under the misleading names `opA` and `opB` (they are type definitions, not operations). A single tiny test helper such as `getFirstField(doc)` would delete those invariant lines, make intent legible, and be reusable by the next tests that get opted into Flow. This is a moderate maintainability concern, not a blocker. See `01_error-module.md`, Finding 3.

The last finding is minor: `locatedError-test.js` (lines 29 and 41) uses `const e: any` so it can attach `locations`, `path` and `nodes`, which switches off type checking on exactly the objects whose shape those tests are about, and `inspect-test.js` (line 31) gains a bare `// $FlowFixMe` with no stated reason. In a newly `@flow strict` file these tend to be copied forward and never revisited. Prefer `Object.assign(new Error(...), {...})` with a small local shape, and give the suppression a one-line justification. See `01_error-module.md`, Finding 4.

## Proposed remediation sequence

1. Restore the no-stack coverage in the 'creates new stack' test (Finding 2), since it is the only item that changes what is actually verified.
2. Bring the `export function GraphQLError` signature in line with the declaration and switch to the `?(...)` idiom (Finding 1).
3. Introduce a small `getFirstField`-style test helper and use it in both error tests (Finding 3).
4. Replace `any` and annotate the `$FlowFixMe` (Finding 4) as cleanup.

## Approval bar

No file crosses 1k lines, no production branching was added, and there is no wrapper or abstraction churn. The presumptive blockers do not apply. I would approve after step 1 and step 2, which are small and local.
