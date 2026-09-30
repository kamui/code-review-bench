# Review blind-52184f

### Item 1
Location: src/error/__tests__/GraphQLError-test.js:57-64
Claim: In `src/error/__tests__/GraphQLError-test.js:57-64`, the PR swaps the fixture `{ message: 'original' }` for `new Error('original')` so it type-checks against `?Error`. A real `Error` always carries a `stack`, so `GraphQLError.js:195` (`if (originalError && originalError.stack)`) now copies the original's stack. The fall-through to `Error.captureStackTrace` that the test title describes is never reached. The only stack assertion, `expect(e.stack).to.be.a('string')`, passes either way, so the test has silently become a duplicate of "uses the stack of an original error". I confirmed this by execution with a scratch mocha probe: with the head fixture, `e.stack === original.stack`. The remedy is to keep a real `Error` but blank its stack with `original.stack = ''`. That value is falsy at runtime but still a `string` for Flow. Then assert `expect(e.stack).to.be.a('string').and.not.equal(original.stack)`, so the test actually proves a fresh stack was created. Full evidence and the probe are in `02_error_and_jsutils_tests.md` (Finding 2.1).
Consequence: —
Fix: —

### Item 2
Location: src/error/GraphQLError.js:25-94
Claim: `src/error/GraphQLError.js` describes the constructor twice: once in `declare class GraphQLError` (line 25 is the changed `nodes` line) and once in the real `export function GraphQLError` (line 94). Flow does not reconcile the two. The PR changes line 25 to `$ReadOnlyArray<ASTNode> | ASTNode | void | null` but leaves line 94 as `... | void`. The implementation already handles `null` via the truthiness check on lines 101–107, so its annotation is now narrower than both its behavior and its public declaration. The pair was already out of sync for `originalError` (line 29 vs. line 98), and this adds a second divergence. The new spelling `| void | null` also departs from the `?T` idiom used by every sibling parameter, even though the stated intent (the author's review comment on line 25) is to make `nodes` uniform with them. The remedy is one shared alias, `type BlameNodes = $ReadOnlyArray<ASTNode> | ASTNode`, used as `nodes?: ?BlameNodes` in both signatures, with `originalError` reconciled at the same time. At minimum, line 94 should mirror line 25 using the `?(...)` form. See `01_error_constructor_contract.md` (Finding 1.1).
Consequence: —
Fix: —

### Item 3
Location: src/error/__tests__/printError-test.js:52-84
Claim: In `src/error/__tests__/printError-test.js:52-84`, the typing work turns what was a one-liner per source into a duplicated four-step block (parse, take `definitions[0]`, `invariant(... Kind.OBJECT_TYPE_DEFINITION && fields)`, take `fields[0]`), written twice. The narrowed nodes are named `opA`/`opB` even though they are object type definitions, not operations, while the same PR uses `operationNode` for a real operation in the sibling test file. `invariant(fieldA && fieldB)` on line 80 is redundant: Flow types an array index read as the element type, and at runtime an `undefined` field would throw on `.type` anyway. `invariant(fieldNode)` at `GraphQLError-test.js:26` is redundant in the same way. The fix is a local `firstFieldTypeNode(body, sourceName)` helper that does the narrowing once and returns `typeDef.fields[0].type`. That deletes the duplication, the misleading temporaries, and the extra invariant, and it restores the original `fieldTypeA`/`fieldTypeB` names so the assertion block is untouched. A worked version is in `02_error_and_jsutils_tests.md` (Finding 2.2).
Consequence: —
Fix: —

### Item 4
Location: src/jsutils/__tests__/inspect-test.js:31-32
Claim: In `src/jsutils/__tests__/inspect-test.js:31-32`, a reasonless `// $FlowFixMe` is added to silence Flow on `String.raw`"\""``, and it also silences any future error on that line. `String.raw` saves exactly one backslash here. `'"\\""'` is the same string and needs no suppression. If the template is kept, the suppression should at least carry a reason, as the repo's other suppressions in `jsutils/` do. This is low severity. See `02_error_and_jsutils_tests.md` (Finding 2.3).
Consequence: —
Fix: —
