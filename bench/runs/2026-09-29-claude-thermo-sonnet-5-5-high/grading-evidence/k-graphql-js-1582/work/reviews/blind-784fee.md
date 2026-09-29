# Review blind-784fee

### Item 1
Location: src/error/GraphQLError.js:25
Claim: In `src/error/GraphQLError.js` line 25 the change appends `| null` to `nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void | null`. Every sibling parameter in the same declaration (`source?: ?Source`, `positions?: ?$ReadOnlyArray<number>`, `path`, `originalError`, `extensions`) uses the `?T` maybe form, so `nodes` is now the only parameter written as a hand-built union with an explicit `void` and `null`. That is a fix that preserves the odd shape rather than removing it: a reader has to notice that `nodes?: X | void | null` means exactly `nodes?: ?X`. Rewrite it as `nodes?: ?($ReadOnlyArray<ASTNode> | ASTNode)`. The behavior and the Flow result are identical, the seven parameters become uniform, and the next person adding a nullable argument copies one idiom instead of two.
Consequence: —
Fix: —

### Item 2
Location: src/error/__tests__/printError-test.js:52-85
Claim: In `src/error/__tests__/printError-test.js` lines 52 to 85 the test now has two nearly identical six-line sequences: parse a `Source`, take `definitions[0]`, `invariant` it is an `OBJECT_TYPE_DEFINITION` with `fields`, then take `fields[0]`. The result is a fourth `invariant`, a second `invariant` on both fields together, and four locals per side. The narrowed variables are called `opA` and `opB`, which reads as "operation" although they are object type definitions, and the original `sourceA` was renamed to `docA` while `Source` objects are still built inline. The code-judo move is a five-line local helper such as `getFirstFieldType(source)` that parses, narrows once, and returns `fields[0].type` with the invariants inside it. The test body then collapses to two calls and one `new GraphQLError(..., [typeA, typeB])`, the trailing `invariant(fieldA && fieldB)` disappears, and the misleading names go with it.
Consequence: —
Fix: —

### Item 3
Location: src/error/__tests__/locatedError-test.js:29-41
Claim: `src/error/__tests__/locatedError-test.js` lines 29 and 41 declare `const e: any = new Error(...)` so the test can bolt on `locations`, `path`, `nodes`, `source`, `positions` and a string `path`. In a file that has just been promoted to `@flow strict` this switches Flow off for exactly the objects the test exists to exercise, and it does so twice with no comment explaining the deliberate type violation. The tests are about duck-typed and hostile error shapes, so the honest boundary is a small typed helper (for example `Object.assign(new Error(msg), { path: '...' })`, or a named `const e: Error & { [string]: mixed }`) that says "this is intentionally not a GraphQLError". Separately, `src/jsutils/__tests__/inspect-test.js` line 31 adds a bare `// $FlowFixMe` above `inspect('"')` with no error code and no reason. The call takes `mixed` and `String.raw` is fine at runtime, so the suppression hides an unexplained Flow complaint and will silently swallow any different error that later appears on that line. State the reason, or sidestep it by writing the expectation as the plain literal `'"\\""'`.
Consequence: —
Fix: —

### Item 4
Location: src/error/__tests__/GraphQLError-test.js:14-26
Claim: `src/error/__tests__/GraphQLError-test.js` lines 14 to 26 replace the per-test parse blocks with a shared `source`, `ast`, `operationNode` and `fieldNode`, and run two `invariant` narrowing checks at module load. Deduplicating is the right direction, but a failing invariant here would abort loading the whole file rather than fail one test, which is a harsher failure mode than the code it replaced. The reformat to `dedent` also forced the expected numbers to be rewritten by hand (`[8]` became `[4]`, column 7 became 3, and the standalone `[10]` became `[6]` at line 91), so the tests now hard-code offsets into a fixture defined thirty lines above and nothing ties `6` to "the second line of `source`". Keep the hoisting, but derive the offsets (for example `source.body.indexOf('field')`) or name the constants next to the fixture, and consider a tiny `getFirstSelection(doc)` helper so the narrowing lives in one function rather than in top-level statements that run on import.
Consequence: —
Fix: —
