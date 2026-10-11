# Thermo-nuclear code quality review — graphql/graphql-js#1582

"Enable Flow typings on errors tests + Fix typing for Error constructor"
Range `5384d218539dbb6bb39b25e0b7a5dcdd69ad8a11..7e39a122eea9292eeffa6905ffdf8a60c5161cfd` (one commit, 5 files, +48 / −47).

## Verdict

Changes requested. The PR is small and its direction is right: four test files move from `@noflow` to `@flow strict`, the `GraphQLError` test stops re-parsing the same three-line document in five places, and the published constructor type finally admits the `null` that callers were already passing. None of the size or spaghetti blockers apply — the largest touched file is 220 lines and no production control flow changes.

It does not clear the bar as written, for one reason that matters and three that are cheap to fix alongside it. Making a test type-check silently changed what that test covers, and the test kept its old name. Separately, the one production line fixes a contract in one of the two places the file states it, a new blanket type suppression is added where deleting four characters of cleverness would have avoided it, and the Flow narrowing in the `printError` test is pasted twice under a wrong name. Each has a concrete, behaviour-preserving remedy below; together they are perhaps twenty lines of change.

## Findings

Ordered by severity. Each is one paragraph here; the linked detail file carries the code, the commands and the worked proposal.

### F1 — "creates new stack if original error has no stack" no longer has a stackless original

In `src/error/__tests__/GraphQLError-test.js:57-64` the fixture `const original = { message: 'original' }` was replaced with `const original = new Error('original')` so that the sixth constructor argument satisfies `?Error`. A real `Error` always has a `stack`, so the constructor now takes the `originalError && originalError.stack` arm at `src/error/GraphQLError.js:195` and copies the original's stack, which is the opposite of what the title says and exactly what the preceding test, `uses the stack of an original error` (lines 41-55), already checks with a stronger assertion. The two tests are now duplicates apart from `null` versus `undefined` placeholders, and the case the title names — an `originalError` is supplied but carries no stack, so `Error.captureStackTrace(this, GraphQLError)` must run — is no longer exercised anywhere. A scratch probe confirmed it: with the base fixture `captureStackTrace` is called once and `e.stack !== original.stack`; with the head fixture it is called zero times and `e.stack === original.stack`. The test still passes because its only stack assertion is `to.be.a('string')`, which holds on either arm. Keep the `Error` so the call type-checks, but make it genuinely stackless — `Object.defineProperty(original, 'stack', { value: undefined })` routes through `captureStackTrace` again in the same probe — after which the existing `to.be.a('string')` assertion means what the title claims; or, if the case is not worth keeping, delete the test rather than leave a weaker duplicate under a misleading name. Detail: [01_graphqlerror_test_fixtures.md](01_graphqlerror_test_fixtures.md).

### F2 — the `nodes` nullability fix is applied to one of two parallel constructor signatures

`src/error/GraphQLError.js` declares the constructor twice: the ambient `declare class` at lines 22-31, which callers are checked against, and the implementing `export function GraphQLError` at lines 92-100, which the body is checked against. The PR widens line 25 to `nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void | null` and leaves line 94 at `nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void`, so the file now states that `null` is both accepted and not accepted for the same parameter, seventy lines apart, with nothing in the toolchain relating the two. The body is null-safe, so this is a contract defect and not a runtime one, but it is the only line in the PR that ships (the file is copied to `dist` as the `.js.flow` definition) and it should not ship half-done. The spelling compounds it: every other nullable parameter in both lists uses the maybe prefix (`?Source`, `?$ReadOnlyArray<number>`, `?Error`), and the sibling at `src/type/validate.js:101` writes the same shape as `$ReadOnlyArray<?ASTNode> | ?ASTNode`, yet this change extends the one outlier to `| void | null`, the longhand for what `?` already means. Change both lines to `nodes?: ?($ReadOnlyArray<ASTNode> | ASTNode)` so the two signatures agree and the parameter list reads uniformly. Detail: [02_graphqlerror_constructor_contract.md](02_graphqlerror_constructor_contract.md).

### F3 — an unexplained `$FlowFixMe` is added where a plain string literal removes the need for it

`src/jsutils/__tests__/inspect-test.js:31` gains a bare `// $FlowFixMe` above ``expect(inspect('"')).to.equal(String.raw`"\""`)`` so that the file can be marked `@flow strict`. The comment gives no reason, issue link or version tag, unlike the suppressions in `src/jsutils/objectValues.js:15`, `isInteger.js:14`, `isFinite.js:14` and `src/subscription/mapAsyncIterator.js:25`, and it blankets the whole following line, which includes the `inspect(...)` call the test exists to check — so on this line the PR turns the type-checker on and off again in the same commit. The tagged template is not earning its place: the expected value is four characters, and ``String.raw`"\""` `` is the same string as `'"\\""'` (confirmed at runtime). Replace the template with the literal and delete the suppression, which reduces this file's diff to the header line; if `String.raw` is kept for readability, give the suppression a reason and move it so it covers only the template. Detail: [04_inspect_test_suppression.md](04_inspect_test_suppression.md).

### F4 — the Flow narrowing in the two-source `printError` test is duplicated and misnamed

In `src/error/__tests__/printError-test.js:62-80` the base's two one-line lookups (`sourceA.definitions[0].fields[0].type`) became two identical three-line blocks — bind `opA`/`opB`, `invariant(op && op.kind === Kind.OBJECT_TYPE_DEFINITION && op.fields)`, bind `fieldA`/`fieldB` — followed sixteen lines after the first binding by a joint `invariant(fieldA && fieldB)`. The narrowing is pasted rather than shared, the proof that each field exists lives away from the binding it guards, and `opA`/`opB` name `ObjectTypeDefinitionNode`s with the abbreviation the sibling test file uses for operation nodes, apparently carried over from it. Extract one local helper that takes a body and a source name, performs the narrowing once with each invariant directly under its binding, and returns `field.type`; the test body then shrinks back to two calls and the `GraphQLError` construction, the `op*` names disappear, and the base's `fieldTypeA`/`fieldTypeB` vocabulary is restored. The `sourceA` → `docA` rename in the same hunk is correct and should stay. Detail: [03_printerror_and_located_tests.md](03_printerror_and_located_tests.md).

## Proposed remediation sequence

1. Fix F1 first, on its own: restore a stackless original in the `GraphQLError` test. It is the only item where the suite currently asserts less than it says it does, and it is independent of everything else.
2. Fix F2 in `GraphQLError.js`: two lines, both signatures, `?(…)` spelling. Do it before touching the tests again, since it is the contract the tests are being typed against.
3. Fix F3 by replacing the `String.raw` template with a literal and dropping the suppression. After this the PR adds no suppressions at all.
4. Fix F4 by extracting the narrowing helper in `printError-test.js`.
5. Re-run `npm run check` and the four test files. Steps 1-4 each change what Flow sees, and Flow was not run in this review.

## What was checked, and how

The four changed test files were run at the head with the clone's own mocha (`--require @babel/register --require @babel/polyfill`, Babel cache disabled so nothing is written into the clone): 27 passing, 0 failing. A scratch mocha file kept outside the clone (`clone-work/scratch/stack-branch-probe.js`) produced the F1 branch counts and the F3 string equivalence. The re-derived offsets and line/column expectations for the hoisted, dedented fixture in `GraphQLError-test.js` were checked by hand and are correct.

`flow check` was not run: the execution allowance for this review covers focused mocha runs only. Every statement above about what Flow accepts or rejects — that the two constructor signatures can disagree silently, that the proposed stackless-`Error` fixture and the proposed helper need no suppression, and what the error under the `$FlowFixMe` actually is — is reasoned from the source and from existing patterns in the tree, not observed. Step 5 of the sequence exists to close that gap.

## Considered and not raised

Hoisting the parsed fixture to module scope in `GraphQLError-test.js` is a good change and matches `getOperationRootType-test.js`. The two `const e: any` annotations in `locatedError-test.js` are a proportionate escape hatch for tests that deliberately build off-contract errors. Carrying a `jsutils` test in an "errors tests" PR is a minor scope stretch. No file approaches the 1000-line threshold, and no branching was added to production code. Reasoning for each is in the detail files.

## Review provenance

Single primary review context on `claude-opus-5-5` at `high`. No child reviewers were started, and no cross-model or alternate-model review was run. Prior reviews, reference findings and upstream discussion were not consulted.
