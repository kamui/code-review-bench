# Flow typing in tests

## Finding

In `src/error/__tests__/locatedError-test.js:29-41`, the two newly strict test cases declare `const e: any` before attaching GraphQLError-shaped fields or the Elasticsearch `path`. This makes every property assignment and subsequent assertion unchecked, even though enabling Flow on these tests is the purpose of this patch. `src/jsutils/__tests__/inspect-test.js:31-32` additionally suppresses Flow on the string expectation with `$FlowFixMe`. These are narrow fixtures, so broad `any` values and a suppression are avoidable escape hatches rather than a necessary abstraction. Model each augmented error with an explicit intersection/object type and construct it from the `Error` and a typed property object (for example, with `Object.assign`); in the inspect test, use a typed expected expression such as `JSON.stringify('"')`. This preserves the runtime cases while letting strict Flow validate the fixture shape and expected value.

## Evidence and scope

The reviewed diff changes the file headers from `@noflow` to `@flow strict` in `locatedError-test.js` and `inspect-test.js`. It adds `any` to both `Error` fixtures at lines 29 and 41, and adds `$FlowFixMe` immediately before the `String.raw` expected-value assertion at line 31. The two `Error` fixtures then set and inspect properties whose intended shapes are visible at the call sites: the first has `locations`, `path`, `nodes`, `source`, and `positions`; the second has a string `path`.

The structural change is limited to test typing. The source change in `GraphQLError.js` adds `null` to the `nodes` constructor annotation, aligning it with the call sites that already pass `null`; it does not add branches or increase implementation complexity. The AST narrowing via `invariant` in the other changed tests is consistent with typed-test patterns in the repository and is not a finding.

## Measurements and commands

The range contains five files and 48 insertions / 47 deletions. No changed file is near 1,000 lines. The inspected command was `git diff --check main...review-head`; it passed. Relevant source evidence was inspected with `git show review-head:<path> | nl -ba` and the exact range with `git diff main...review-head`.

## Verification status

No tests or Flow checks were run. The review task did not request test execution. The execution policy permitted focused tests, but no test result is claimed here.

## Worked code-judo proposal

Keep the fixture behavior and let its shape carry the type information: build the first fixture as an `Error` intersected with its GraphQLError-like fields, initialized from a typed object; build the Elasticsearch fixture as an `Error` intersected with `{ path: string }`. Use a direct typed expression for the inspected string's JSON representation, such as `JSON.stringify('"')`, instead of relying on a Flow suppression around `String.raw`. This removes all three escape hatches without introducing a helper or changing what the tests exercise.
