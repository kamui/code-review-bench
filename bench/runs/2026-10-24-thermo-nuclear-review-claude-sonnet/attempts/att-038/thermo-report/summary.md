# Thermo nuclear review: graphql-js #1582

Range: `5384d21..7e39a12`. The change enables Flow on five test files and widens the `nodes` parameter in the `GraphQLError` constructor declaration. I ran mocha on `src/error/__tests__/*` and `inspect-test.js`: 27 tests pass. `flow check` reports 0 errors. Nothing in the diff changes runtime library code. The only non-test edit is a Flow `declare class` signature that adds `| null` to `nodes`. That matches existing callers, which already pass `null`.

## Finding 1 (Low): the "no stack" test no longer exercises the no-stack branch

File: `src/error/__tests__/GraphQLError-test.js`, lines 57-65 (the test `creates new stack if original error has no stack`).

The diff changes `const original = { message: 'original' };` to `const original = new Error('original');`. The test exists to cover the fallback in `src/error/GraphQLError.js` lines 194-210. That fallback is taken only when `originalError` is falsy or has no `.stack`: `Error.captureStackTrace`, or `Error().stack` where that is missing.

An `Error` instance always has a `.stack`. The test therefore now takes the first branch, `originalError.stack`, which the test above it (`uses the stack of an original error`) already covers. The assertions are only `e.stack` is a string, plus name, message and `originalError`. They pass on either branch. The test name now describes behaviour it does not test.

I confirmed this by reading `GraphQLError.js` lines 195-210 and by running the suite, which passes. No test remains that passes a stack-less `originalError`. If the fallback branch regressed (for example, if `captureStackTrace` was dropped), the suite would stay green.

Impact: a loss of test coverage, with no runtime effect. The motivation was Flow, because `originalError` is typed `?Error` and the object literal failed to typecheck. A typed stack-less error would keep the coverage, for example `const original = new Error('original'); delete original.stack;` or `Object.defineProperty`, or a cast with `any` as the same PR does in `locatedError-test.js`.

## Observations (not actionable)

- The `GraphQLError-test.js` fixtures now share a module-level `Source` built with `dedent`. The position and location expectations were updated to match: `[4]` / `{line: 2, column: 3}`, and `[6]` / `column: 5`. The tests pass, so the arithmetic is consistent.
- `new GraphQLError()` with no arguments became `new GraphQLError('str')`. That is correct for the declared signature, so no behaviour is lost.
- `// $FlowFixMe` in `inspect-test.js` suppresses an `String.raw` typing complaint. This is harmless.
