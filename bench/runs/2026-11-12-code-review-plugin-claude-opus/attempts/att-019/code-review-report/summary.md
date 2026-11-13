# Code review — graphql/graphql-js#1582

"Enable Flow typings on errors tests + Fix typing for Error constructor" (range `5384d218..7e39a122`)

Found 1 issue (validated in step 5 and kept in step 6). No CLAUDE.md files apply to the changed paths.

## 1. "creates new stack if original error has no stack" test no longer exercises the no-stack path

- **Reason flagged:** bug (a logic error in the test)
- **File / lines:** `src/error/__tests__/GraphQLError-test.js` lines 57-64. The code path it should cover is `src/error/GraphQLError.js` lines 195-209.
- **Description:** The PR changed `const original = { message: 'original' };` to `const original = new Error('original');`. That makes the test give `GraphQLError` an original error that *does* have a stack.
- **Why it fails:** A `new Error(...)` always has a `stack` string. So the constructor's `if (originalError && originalError.stack)` branch (`GraphQLError.js:195`) is taken, and the error just reuses `originalError.stack`. The `Error.captureStackTrace` / `Error().stack` fallback paths that the test is named for never run. The test's only stack assertion, `expect(e.stack).to.be.a('string')`, passes either way. The test is now a weaker copy of the preceding "uses the stack of an original error" test, and it would still pass if the no-stack fallback were broken. Fix: keep the scenario while satisfying Flow, e.g. create the `Error` and then `delete` its `stack` (typed as `any`), or cast a plain object to `any`.
