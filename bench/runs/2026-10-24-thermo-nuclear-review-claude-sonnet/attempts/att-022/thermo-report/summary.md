# Thermo Nuclear Review: graphql-js#1582

No actionable findings.

Scope reviewed: `git diff main...review-head` (5 files). The only runtime-adjacent change is the Flow declaration of the `GraphQLError` constructor in `src/error/GraphQLError.js`, which now accepts `null` for `nodes`. The existing implementation already treats a falsy `nodes` value the same as `undefined`, and callers already pass `null` in the tests. The change widens a type to match runtime behavior and has no effect on emitted code.

The rest of the diff is test-only (`@noflow` changed to `@flow strict`, shared fixtures, `invariant` narrowing, `any` annotations, and one `$FlowFixMe` in `inspect-test.js`). The recomputed positions and locations in `GraphQLError-test.js` (offset 4, line 2 column 3, and so on) match the new dedented source. Verification: the mocha run for `src/error/__tests__` and `inspect-test.js` passed 27 tests, and `flow check` reported 0 errors. I found no security, devex or feature-gate concerns.
