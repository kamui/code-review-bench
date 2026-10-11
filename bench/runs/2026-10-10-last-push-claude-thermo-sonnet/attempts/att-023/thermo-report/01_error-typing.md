# 01 — Error module Flow typing and test conversion (detail)

Range reviewed: `5384d218..7e39a122` (`git diff main...review-head`), 5 files, +48/−47.

## Measurements and commands

- `git diff main...review-head` — read in full.
- `./node_modules/.bin/mocha --require @babel/register --require @babel/polyfill src/error/__tests__/*.js src/jsutils/__tests__/inspect-test.js` — 27 passing.
- `./node_modules/.bin/flow check` — `Found 0 errors`.
- No file crosses 1000 lines. No production logic was added; the only source change is one type annotation.

## Finding 1 — The `GraphQLError` declaration and its implementation signature now disagree on `nodes`

`src/error/GraphQLError.js:25` widens the `declare class` constructor to accept `nodes?: ... | void | null`. The implementing function at `src/error/GraphQLError.js:94` still declares `nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void`. The implementation handles null correctly at runtime, because `Array.isArray(nodes) ? ... : nodes ? [nodes] : undefined` treats null like undefined. The two signatures for one constructor now say different things, though. Flow does not catch the drift because the function is cast through `any` when the prototype is assigned and the exported name is typed by the `declare class`.

The tests now pass `null` (`new GraphQLError('msg', null, source, [6])`, `new GraphQLError('msg', null, null, null, null, original)`), and the declaration was widened to make that typecheck. The author widened one of the two copies of the contract. The real fix is to make the implementation signature the single source of truth: add `| null` to line 94 too, or stop duplicating the parameter list. Leaving the implementation unchanged means the next person who edits one signature will not know the other exists.

Code-judo proposal: the other trailing parameters (`source`, `positions`, `path`, `originalError`) already use `?T`, which admits null. `nodes` is the odd one out. Normalize it to `nodes?: ?($ReadOnlyArray<ASTNode> | ASTNode)` in both places. That makes the optional-versus-nullable convention uniform and deletes the special case the PR just encoded.

Verification: confirmed by reading lines 22–31 and 92–100 of `src/error/GraphQLError.js`. Flow passes, so this is a maintainability finding, not a type failure.

## Finding 2 — "creates new stack if original error has no stack" no longer tests the branch it names

Before the PR the test used `const original = { message: 'original' }`, a plain object with no `stack`. That exercised the `else if (Error.captureStackTrace)` branch at `src/error/GraphQLError.js:195-208`. To satisfy Flow (`originalError?: ?Error`), the PR changed it to `new Error('original')`, which has a `stack`. The test now takes the `originalError.stack` branch, the same branch the preceding test, "uses the stack of an original error" (`GraphQLError-test.js:41-54`), covers. Its only stack assertion is `to.be.a('string')`, so it passes either way.

The two tests are now near-duplicates. The no-stack branch (`captureStackTrace` or the `Error().stack` fallback) has lost its only coverage, and the test title now describes behavior the test does not exercise. Making a test typecheck cost the PR its coverage, and the suite stays green so nobody will notice.

Remedy: build an error with no stack, for example `const original = new Error('original'); original.stack = undefined;` typed through one narrow cast, or `Object.defineProperty`. Then assert `e.stack !== undefined` and `e.stack` does not equal `original.stack`. If the project decides the case is impossible under the `Error` type, delete the test rather than leaving a misleading duplicate.

Verification: confirmed by reading the pre- and post-PR test bodies and the implementation branch order.

## Finding 3 — Shared AST fixtures built and asserted at module load in `GraphQLError-test.js`

`src/error/__tests__/GraphQLError-test.js:17-26` now builds `source`, `ast`, `operationNode` and `fieldNode` at module scope and runs `invariant(...)` there. The old tests each built their own source inside the `it` block. Three consequences:

1. A parse or shape failure now throws at import time, which kills the whole file with a stack trace pointing at module load and not at a named test.
2. The tests are coupled through shared offsets. `[8]` became `[4]`, `{line: 2, column: 7}` became `{line: 2, column: 3}`, and the bare `[10]` position became `[6]`. These offsets are now meaningful only relative to the shared fixture, and `[6]` in "converts source and positions to locations" is a magic number with no visible link to the fixture string.
3. The `invariant(...)` calls exist only to satisfy Flow refinement. They do no verification, and they sit at module scope as test infrastructure.

Remedy: put the fixture in a small `function getFixture()` helper, or in a `before` block with typed `let` bindings. Alternatively derive the position from the fixture (`source.body.indexOf('field')`) so the offset documents itself. This is minor next to Finding 2, but it was a deliberate restructuring and it made the tests harder to read.

## Finding 4 — Repeated `invariant`-refinement boilerplate and misleading names in `printError-test.js`

`src/error/__tests__/printError-test.js:62-80` repeats the same five-line refine-and-extract pattern twice: `invariant(op && op.kind === Kind.OBJECT_TYPE_DEFINITION && op.fields)`, then `fields[0]`, then a third `invariant(fieldA && fieldB)`. The variables are named `opA` and `opB`, but they are object type definitions, not operations. The `sourceA`/`sourceB` to `docA`/`docB` rename is a reasonable fix, but `op*` was introduced in the same change and is wrong.

Remedy: extract one local helper, `function firstFieldType(source: Source)`, that parses, refines and returns `fields[0].type`. The two call sites then shrink to one line each and the third invariant disappears. Name the intermediates `typeDefA` and `fieldA`.

## Finding 5 — Untyped escape hatches: `e: any` and an unexplained `$FlowFixMe`

`src/error/__tests__/locatedError-test.js:29,41` annotate `const e: any = new Error(...)` so the test can attach `locations`, `path` and so on. The `any` is deliberate here, because these tests build deliberately malformed GraphQLError-like objects. A `@flow strict` file is meant to avoid unclear types, though, and the cast also removes checking from the `locatedError(e, ...)` calls. A narrower `Error & { path: mixed, ... }` or a comment explaining why the test needs `any` would show the intent.

`src/jsutils/__tests__/inspect-test.js:31` adds a bare `// $FlowFixMe` over `expect(inspect('"')).to.equal(String.raw\`"\""\`)`. It has no explanation of what Flow rejects. `String.raw` with a template literal is a known Flow gap, so a one-line comment or a plain string literal (`'"\\""'`) would remove the suppression altogether. The plain literal is the better option because it deletes a suppression instead of documenting one.

## Non-findings

- `Kind.OPERATION_DEFINITION` narrowing and `invariant` usage follow existing conventions in the repository.
- `dedent` for the fixture is the canonical helper.
- `@noflow` to `@flow strict` is the intended direction of the surrounding series of PRs.
