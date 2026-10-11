# Detail: src/error tests, GraphQLError typing, inspect-test

Scope: `git diff main...review-head` (5 files, +48/-47). Commands run: `git diff main...review-head`; `mocha --require @babel/register --require @babel/polyfill src/error/__tests__/*.js src/jsutils/__tests__/inspect-test.js` (27 passing); `git grep FlowFixMe -- src`; `grep -n "opA\|opB\|invariant(" ...`; `sed -n 195,210p src/error/GraphQLError.js`.
No file approaches 1000 lines; no file-size concern. Flow itself was not run (no flow binary use was in the allowance), so Flow-typing claims are by reading.

## F1. "creates new stack if original error has no stack" no longer tests the no-stack path (verified by reading + source)

Evidence: `src/error/__tests__/GraphQLError-test.js:57-64`. The diff replaces `const original = { message: 'original' }` (a plain object with no `stack`) with `const original = new Error('original')`. Every `Error` instance has a `stack`. In `src/error/GraphQLError.js:195-209` the constructor takes the `originalError.stack` branch whenever `originalError && originalError.stack` is truthy, so the test now runs the same branch as the preceding test "uses the stack of an original error" (line 40). The `else if (Error.captureStackTrace)` / `else` branches are now only reached by tests that pass no originalError. The test name still claims otherwise, and its assertions (`e.stack` is a string) pass trivially. Mocha passes, so CI will not catch it.

This is the typing cleanup quietly deleting coverage: the Flow-motivated change (the constructor now declares `originalError?: ?Error`) was resolved by making the fixture satisfy the type instead of making the fixture an Error that lacks a stack.

Remedy: build a real Error and remove its stack, e.g.

    const original = new Error('original');
    original.stack = undefined; // or: delete original.stack, with a $FlowFixMe if Flow objects
    const e = new GraphQLError('msg', null, null, null, null, original);
    expect(e.stack).to.be.a('string');
    expect(e.stack).to.not.equal(original.stack);   // strengthens the assertion

Alternatively drop the test, since it is now a duplicate, but then the branch is untested.

## F2. printError-test.js: misleading names and duplicated narrowing boilerplate (verified by reading)

Evidence: `src/error/__tests__/printError-test.js:59-80`. The diff renames `sourceA`/`sourceB` to `docA`/`docB` (good, they were documents) but then names `definitions[0]` as `opA`/`opB`. These are `ObjectTypeDefinition` nodes (the invariant checks `Kind.OBJECT_TYPE_DEFINITION`), not operations; the name was apparently copied from the `operationNode` pattern in GraphQLError-test.js. The two blocks are also copy-pasted: parse, `definitions[0]`, invariant on kind and `fields`, take `fields[0]`, and then a second `invariant(fieldA && fieldB)` because `fields[0]` is possibly undefined. That is three invariants and six lines to reach one `.type` per document.

Code-judo proposal: a four-line local helper in the test file, e.g. `function getFirstFieldType(doc)` that does the narrowing once and returns `fields[0].type`, called as `getFirstFieldType(docA)`/`getFirstFieldType(docB)`. The test body then returns to its pre-PR shape, the second invariant disappears, and the misnamed variables with it. The same narrowing idiom (definitions[0] + kind invariant + first selection) now appears in GraphQLError-test.js too, so a shared helper under a test utility is a further option if more `@flow strict` conversions follow this pattern, as the series of recent "Enable Flow typings on ... tests" commits suggests they will.

## F3. `const e: any` in locatedError-test.js defeats the purpose of `@flow strict` (verified by reading)

Evidence: `src/error/__tests__/locatedError-test.js:29` and `:41`. Flipping the header from `@noflow` to `@flow strict` and then annotating the fixtures `any` silences the checker on exactly the values the tests are about (an Error with `locations`/`path`/`nodes` bolted on, and an Error with a string `path`). `any` is also not a "strict"-friendly annotation (flow strict commonly enables unclear-type lint). Prefer constructing the fixture with `Object.assign(new Error('...'), { locations: [], path: [], nodes: [] })`, which Flow types as an intersection without `any`, or keep one narrowly commented `// $FlowFixMe` suppression per fixture so the intent ("deliberately wrong shape") is documented.

## F4. Module-level shared fixture executed at import time, with magic offsets (verified by reading)

Evidence: `src/error/__tests__/GraphQLError-test.js:14-26` parses a document and runs two `invariant(...)` calls at module top level, and the tests reuse `source`, `operationNode`, `fieldNode` across the file. Consequences: a parser failure or invariant failure aborts the whole file at load with an opaque stack instead of failing one test; the parsed AST is shared mutable state between tests (the tests do `deep.equal` on nodes, so mutation would cross-contaminate); and the expectations moved from readable-source offsets to bare numbers (`positions` `[4]`, `[6]`, `column: 3`, `column: 5`) that a reader must recompute from the dedent template. This is a consistency choice (the repo's other converted tests use per-test sources), not a defect, but the five deleted per-test copies were the more self-contained form. A middle path is a small `beforeEach`-free factory `function getFixture()` returning `{source, operationNode, fieldNode}`, or keeping the shared constants but adding a one-line comment on the offsets.

Note also the "serializes to include message and locations" expectation changed from line 1/column 3 to line 2/column 3 because it now uses the shared fixture; the original test for a one-line source with its own parse is no longer covered (minor).

## F5. Unexplained `// $FlowFixMe` in inspect-test.js (verified by reading)

Evidence: `src/jsutils/__tests__/inspect-test.js:31`, `// $FlowFixMe` above `expect(inspect('"')).to.equal(String.raw\`"\""\`)`. Every other `$FlowFixMe` in `src` (`isFinite.js:14`, `isInteger.js:14`, `objectValues.js:15`, `mapAsyncIterator.js:25,60`) carries a reason or tracking link; this one gives none, and the `@noflow` to `@flow strict` flip of this file has no connection to the error work in the rest of the commit. Remedy: either replace `String.raw` with a plain `'"\\""'` literal (deleting the suppression and the magic) or state the Flow limitation in the comment.

## F6. `nodes?: ... | void | null` hand-widens a union instead of using the file's nullable idiom (verified by reading)

Evidence: `src/error/GraphQLError.js:25`. Every other optional parameter in the declaration uses the `?T` maybe form (`source?: ?Source`, `positions?: ?$ReadOnlyArray<number>`, `path?: ...`). The change appends `| void | null` to `nodes`, which is redundant (`nodes?:` already admits `void`) and inconsistent. Prefer `nodes?: ?($ReadOnlyArray<ASTNode> | ASTNode)`. Behavior is identical; it keeps the declaration uniform and removes the redundant `void`. Also worth checking that the implementation signature further down (line ~95) is kept in sync, since the declared-class/implementation pair is the type boundary here.
