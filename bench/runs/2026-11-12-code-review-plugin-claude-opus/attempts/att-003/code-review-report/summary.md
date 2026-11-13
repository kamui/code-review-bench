# Code review — graphql/graphql-js#1582

"Enable Flow typings on errors tests + Fix typing for Error constructor" (range `5384d218..7e39a122`, `main...review-head`)

Step 1 did not stop the review: the PR is not a draft and is not trivial. No CLAUDE.md files exist in the repository, so the CLAUDE.md compliance check found nothing. Step 6 kept 1 issue.

## Issues

### 1. "creates new stack if original error has no stack" test no longer exercises the no-stack path

- **Reason flagged:** bug (test logic error introduced by this PR)
- **File / lines:** `src/error/__tests__/GraphQLError-test.js` lines 57-64. The relevant logic is in `src/error/GraphQLError.js` lines 194-207.
- **Description:** The PR changes the test's original error from the plain object `{ message: 'original' }` to `new Error('original')`.
- **Why it fails:**
  - A real `Error` always has a string `stack`. So in `GraphQLError.js` the `if (originalError && originalError.stack)` check is now always true, and the original stack is copied.
  - As a result, the fallback branches that create a new stack (`Error.captureStackTrace` / `Error().stack`) are never reached by this test.
  - The test only asserts `expect(e.stack).to.be.a('string')`, so it still passes. In practice it now duplicates the earlier "uses the stack of an original error" test.
  - The no-stack case named in the test has lost its only coverage. A regression in that branch would go undetected.
  - Flow typing can be satisfied without losing coverage. For example, cast the plain object (`({ message: 'original' }: any)`), or remove the stack from a real `Error`.
