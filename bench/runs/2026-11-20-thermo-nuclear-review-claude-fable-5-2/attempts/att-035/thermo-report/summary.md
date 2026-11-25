# Thermo-nuclear review — graphql/graphql-js#1582

"Enable Flow typings on errors tests + Fix typing for Error constructor"
Range reviewed: `5384d218539dbb6bb39b25e0b7a5dcdd69ad8a11..7e39a122eea9292eeffa6905ffdf8a60c5161cfd` (5 files, +48 / −47).

## Verdict

One actionable finding, low priority, confined to a test. Nothing in the range changes runtime behaviour of the library: the only non-test edit is a Flow-only widening of the declared `GraphQLError` constructor type. No security, feature-gate or developer-workflow impact.

## Findings

### 1. [Low] "creates new stack if original error has no stack" no longer tests a stackless original error

**File:** `src/error/__tests__/GraphQLError-test.js`, lines 57–64 (the changed line is 58).

To satisfy Flow, the fixture was changed from `const original = { message: 'original' };` to `const original = new Error('original');`. A real `Error` always carries a `stack` string, so the constructor now takes the first branch at `src/error/GraphQLError.js:195` (`if (originalError && originalError.stack)`) and copies the original's stack, instead of falling through to `Error.captureStackTrace(this, GraphQLError)` at lines 201–202, which is the path the test is named for. The test is now a weaker duplicate of the preceding one, "uses the stack of an original error" (lines 41–55), and its only stack assertion, `expect(e.stack).to.be.a('string')`, passes on either branch.

Evidence: a scratch probe run against the head with the clone's mocha showed that with the new fixture `e.stack === original.stack` is `true` and the stack begins `Error: original`; with the old `{ message: 'original' }` fixture the stack is a newly captured one beginning `msg`. The four changed test files pass at the head (27 passing), so this is not visible as a failure.

Consequence: the case "an `originalError` is supplied but has no `stack`" is no longer covered anywhere. A regression in that fallback — for example a change that leaves `stack` undefined, or copies `undefined` from the original, when a resolver throws a non-`Error` object or an error whose stack was stripped — would pass the suite. The capture branch itself is still executed by tests that pass no original error at all, so line coverage will not flag the loss.

Remedy direction: keep the fixture stackless while satisfying Flow, for example by casting the plain object (`(({ message: 'original' }: any): Error)`) or by building an `Error` and removing its `stack`, and assert that `e.stack` differs from the original's.

## Checked and found sound

- `src/error/GraphQLError.js:25` — adding `| null` to `nodes` in the `declare class` matches runtime behaviour: `Array.isArray(null)` is false and the `nodes ? [nodes] : undefined` fallback at lines 102–108 yields `undefined`, the same as omitting the argument. The widening is non-breaking for consumers of the published `.flow` files. The implementing function's own annotation at line 94 still reads `| void` without `null`; this is a cosmetic inconsistency with no behavioural effect and is not raised as a finding.
- New expected positions and locations in `GraphQLError-test.js` are right for the shared dedented source `"{\n  field\n}\n"`: `field` starts at offset 4 (line 2, column 3), offset 6 is line 2, column 5, and the operation starts at offset 0 (line 1, column 1). Hoisting the source and AST to module scope is safe; no test mutates them.
- `printError-test.js` — the refactor still passes the two field *type* nodes (`fieldA.type`, `fieldB.type`), so the expected `2:10` output is unchanged.
- `locatedError-test.js` — `e: any` annotations only; `flowtype/no-weak-types` is configured with `any: false`, so lint permits it.
- `inspect-test.js` — only a `// $FlowFixMe` suppression, which matches the `suppress_comment` pattern in `.flowconfig`.
- Single-argument `invariant(...)` calls follow existing usage in `src/jsutils/dedent.js:20` and `src/utilities/introspectionFromSchema.js:35`.

## Not verified

`flow check` and `eslint` were not run; the execution allowance for this review covers mocha only. The Flow-correctness of the newly annotated test files is therefore assessed by reading, not by running the type checker.
