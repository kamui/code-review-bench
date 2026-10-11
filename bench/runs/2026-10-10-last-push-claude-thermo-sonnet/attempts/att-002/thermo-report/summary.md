# Thermo-nuclear review: graphql-js PR #1582 "Enable Flow typings on errors tests + Fix typing for Error constructor"

Range: `5384d21..7e39a12` (1 commit, 5 files, +48/-47). Detail: `01_error-tests-and-typing.md`. Focused mocha run on the touched test files: 27 passing. Flow was not run.

## Verdict

Not approvable as is, though nothing here is structurally large. This is a small typing-cleanup PR: no file nears 1k lines, no new production branching, and the single production change is a one-token widening of a `declare class` signature. The problems are in how the tests were bent to satisfy Flow. One conversion silently deleted the behavior a test claims to cover, and several others satisfy the checker with `any`, copy-pasted narrowing, or a shared import-time fixture instead of making the tests simpler.

## Findings

**1. The "creates new stack if original error has no stack" test no longer tests that.** In `src/error/__tests__/GraphQLError-test.js:57-64` the fixture changed from `{ message: 'original' }` (no stack) to `new Error('original')`, which always has a stack. `GraphQLError.js:195` then takes the `originalError.stack` branch, exactly as the preceding test at line 40 does, so the `captureStackTrace`/`Error().stack` fallbacks are no longer exercised by anything that supplies an originalError, and the test passes vacuously under its old name. The type fix was achieved by weakening the fixture rather than building an Error without a stack (`original.stack = undefined`, with an explicit assertion that `e.stack` differs from it). This is the one finding that costs real coverage and should be fixed before merge.

**2. `printError-test.js` has misnamed, copy-pasted narrowing.** At `src/error/__tests__/printError-test.js:62-80` the object-type-definition nodes are named `opA`/`opB` (they are not operations), and two identical blocks of parse, index, kind-invariant, fields-invariant, index, then a third invariant reach one `.type` each. A four-line local helper that returns the first field's type from a document removes both invariants blocks and the misleading names and restores the test body to its pre-PR shape. If more `@flow strict` test conversions follow with the same `definitions[0]` plus kind-invariant pattern, that helper belongs in a shared test utility rather than being re-derived per file.

**3. `const e: any` in `locatedError-test.js:29,41` defeats the point of `@flow strict`.** The fixtures are deliberately malformed errors, and annotating them `any` simply turns the checker off where it matters, and `any` is hostile to strict mode generally. Build them with `Object.assign(new Error(...), {...})` or document one narrow `$FlowFixMe` per fixture.

**4. `GraphQLError-test.js` now builds a shared parsed fixture at module top level (lines 14-26) with two import-time invariants.** A load-time failure takes down the whole file with an opaque stack, the shared AST is mutable state across tests, and the expectations became bare offsets (`4`, `6`, column `3`, `5`) that are harder to read than the old per-test sources. Consolidating five duplicate setups is a fair goal, but a small factory function or a comment on the offsets keeps the gain without the import-time coupling. The "serializes to include message and locations" test also quietly moved from a one-line source to the shared fixture, changing what it covers.

**5. An unexplained `// $FlowFixMe` was added in `src/jsutils/__tests__/inspect-test.js:31`,** in a file whose only connection to the PR is the header flip. Every other suppression in `src` carries a reason. Replace the `String.raw` expression with a plain literal and delete the suppression, or say what Flow limitation it works around. Converting this file also looks unrelated to "errors tests" in the title and could be its own change.

**6. `nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void | null` in `src/error/GraphQLError.js:25` hand-widens the union** rather than using the maybe-type idiom that every neighbouring parameter uses; `void` is redundant under `nodes?:`. Write `nodes?: ?($ReadOnlyArray<ASTNode> | ASTNode)`, and confirm the implementation signature (~line 95) stays consistent with the declaration.

## Proposed remediation sequence

First restore the stack-fallback coverage (finding 1), since it is a correctness-of-tests regression. Second, replace the printError copy-paste with one helper and fix the names (finding 2). Third, remove the `any` annotations (finding 3) and rewrite the `$FlowFixMe` as a literal (finding 5). Fourth, tidy the declaration (finding 6) and decide whether the shared fixture in GraphQLError-test.js should become a factory (finding 4).

## Approval status

Blocked on finding 1 (lost coverage); findings 2-6 are maintainability requests the author may justify or defer. No file-size, wrapper, or architecture-boundary issues were found. No reviewer other than this single reviewer was used; all work was on `claude-sonnet-5-5` at high.
