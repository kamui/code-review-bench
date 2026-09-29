# Review blind-25b702

### Item 1
Location: src/error/__tests__/GraphQLError-test.js:57
Claim: The most important finding is in `src/error/__tests__/GraphQLError-test.js` at line 57. The test named 'creates new stack if original error has no stack' used to pass a plain `{ message: 'original' }` object so that `originalError.stack` was absent. To satisfy Flow, the fixture became `new Error('original')`, which always has a stack. The test therefore now walks the same `originalError.stack` branch as the preceding test and no longer covers the `Error.captureStackTrace` / `Error().stack` fallback in `GraphQLError.js`; the assertions still pass, so the coverage loss is silent while the title claims otherwise. The remedy is to keep the well-typed fixture but remove its stack (for example `delete original.stack`, or a narrowly commented `$FlowFixMe`) and to assert the resulting stack is not the original's. A type-driven fixture change should not be allowed to change what a test proves. Details are in `01_error-module.md`, Finding 2.
Consequence: —
Fix: —

### Item 2
Location: src/error/GraphQLError.js:25-94
Claim: The second finding is in `src/error/GraphQLError.js` at lines 25 and 94. The diff adds `| void | null` to `nodes` in the `declare class` constructor but leaves the actual `export function GraphQLError(...)` signature 70 lines below unchanged, so the two hand-copied declarations of one contract now disagree (and `originalError` already disagreed between them before this change). Runtime already tolerates `null`, so this is stale typing rather than a bug, but it is exactly the drift a duplicated signature invites. Update both in the same change, use the `?(...)` idiom that the neighbouring parameters use instead of a bespoke `| void | null` union, and consider whether the duplicated types can be collapsed so only one copy exists. See `01_error-module.md`, Finding 1.
Consequence: —
Fix: —

### Item 3
Location: src/error/__tests__/GraphQLError-test.js:14-26
Claim: The third finding concerns the shape of the new test fixtures in `GraphQLError-test.js` (lines 14–26) and `printError-test.js` (lines 62–80). Hoisting the repeated `parse` calls into module-level constants is an improvement, but the narrowing is done with import-time `invariant` calls (a fixture failure kills the whole file at load), every position and column assertion was recomputed by hand and is now coupled to a `dedent` layout defined far away, and `printError-test.js` writes the same three-clause narrowing twice under the misleading names `opA` and `opB` (they are type definitions, not operations). A single tiny test helper such as `getFirstField(doc)` would delete those invariant lines, make intent legible, and be reusable by the next tests that get opted into Flow. This is a moderate maintainability concern, not a blocker. See `01_error-module.md`, Finding 3.
Consequence: —
Fix: —

### Item 4
Location: src/error/__tests__/locatedError-test.js:29-41
Claim: The last finding is minor: `locatedError-test.js` (lines 29 and 41) uses `const e: any` so it can attach `locations`, `path` and `nodes`, which switches off type checking on exactly the objects whose shape those tests are about, and `inspect-test.js` (line 31) gains a bare `// $FlowFixMe` with no stated reason. In a newly `@flow strict` file these tend to be copied forward and never revisited. Prefer `Object.assign(new Error(...), {...})` with a small local shape, and give the suppression a one-line justification. See `01_error-module.md`, Finding 4.
Consequence: —
Fix: —
