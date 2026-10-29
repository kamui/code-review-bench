# Thermo-nuclear review: graphql/graphql-js#1582

"Enable Flow typings on errors tests + Fix typing for Error constructor"
Range `5384d218539dbb6bb39b25e0b7a5dcdd69ad8a11..7e39a122eea9292eeffa6905ffdf8a60c5161cfd`, 5 files, +48 / -47.

## What the change does

The PR flips four test files from `@noflow` to `@flow strict` and adjusts them so they type-check. The only non-test change is one line in `src/error/GraphQLError.js`: the `declare class` constructor's `nodes` parameter gains `| null`, so callers (including the tests) may pass `null` explicitly.

## What I verified

- **Runtime behaviour of the type widening.** The `GraphQLError` implementation already treats `null` nodes as "no nodes": `Array.isArray(null)` is false and `null` is falsy, so `_nodes` becomes `undefined`. I confirmed with a scratch script that `new GraphQLError('msg', null)` yields `nodes === undefined` and `locations === undefined`. The declaration change therefore only widens the accepted type to match what the code already handles; there is no behavioural change, no public API break, and no security surface.
- **Flow.** `flow check` on the head tree reports `Found 0 errors`, so the four newly-typed test files and the declaration change are consistent.
- **Tests.** Running the four touched test files with the clone's mocha gives 27 passing, 0 failing.
- **Position arithmetic in `GraphQLError-test.js`.** The source text moved from an inline literal to `dedent`, which yields `{\n  field\n}\n`. `field` starts at offset 4 (line 2, column 3) and offset 6 is line 2, column 5. The updated expectations ([4], [6], and the JSON string) match this exactly.
- **`printError-test.js`.** The rename from `fieldTypeA/B` to `fieldA.type`/`fieldB.type` is equivalent; the `invariant` guards on `kind === OBJECT_TYPE_DEFINITION` and `fields` only narrow the type and cannot fail for the given fixtures.
- **`locatedError-test.js` and `inspect-test.js`.** `e: any` and the `$FlowFixMe` on `String.raw` are typing-only edits with no change to what is asserted.
- **Devex / feature gates.** No scripts, env vars, ports, dependencies, or flags change. No gated feature is exposed.

## Findings

### Low — `GraphQLError-test.js` lines 57–64: the "creates new stack if original error has no stack" test no longer exercises the no-stack branch

Before this PR the test passed `{ message: 'original' }` as `originalError`, an object with no `stack` property. That input drives the `else if (Error.captureStackTrace)` branch at `src/error/GraphQLError.js:201` while `originalError` is set, which is precisely what the test name claims to cover. To satisfy Flow's `?Error` annotation the PR replaced it with `new Error('original')`, which does have a `stack`. With that input the constructor takes the `originalError && originalError.stack` branch at line 195 and simply copies the original stack, so this test now asserts the same thing as the preceding "uses the stack of an original error" test, just with `null` instead of `undefined` for the middle arguments. I confirmed via a scratch run that the no-stack input (`{ message: 'original' }`) and the `Error` input hit different branches. The consequence is a silent loss of coverage: a future regression in the fallback stack-capture path when an original error lacks a stack (for example an error-like object thrown from a resolver) would not be caught by this suite, and the test name now misdescribes what is tested. A typed alternative that keeps the intent is to build a real `Error` and then clear its stack (for example `const original = new Error('original'); original.stack = undefined;` with an `any` cast, or `delete original.stack`), or to cast the plain object with `(({ message: 'original' }: any): Error)`. This is a test-quality issue only; production code is unaffected.

## Notes (not actionable)

- The widening to `| null` was applied only to the `declare class` constructor signature (line 25). The implementing function's own `nodes` annotation at `src/error/GraphQLError.js:94` still reads `$ReadOnlyArray<ASTNode> | ASTNode | void`. Flow uses the `declare class` for consumers so this has no effect on type checking (confirmed by the clean `flow check`), but the two signatures now differ cosmetically.
