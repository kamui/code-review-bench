# Review blind-9e6824

### Item 1
Location: src/error/__tests__/GraphQLError-test.js:58
Claim: The test 'creates new stack if original error has no stack' now passes `new Error('original')`, which does have a stack, so it no longer tests what its name says.
Consequence: The old fixture `{ message: 'original' }` had no `stack`, forcing the fallback at GraphQLError.js:201 when an originalError is present. With a real Error the `originalError.stack` branch at line 195 is taken instead, duplicating the previous test. A regression that leaves `e.stack` undefined for a stack-less originalError would now go unnoticed; the assertion `e.stack` is a string passes trivially.
Fix: —

### Item 2
Location: src/error/GraphQLError.js:25
Claim: Only the `declare class` constructor was widened to accept `null` for `nodes`; the implementing function at line 94 still declares `nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void`.
Consequence: The two signatures for the same constructor now disagree. Callers are typed against the declaration and may pass `null`, while the body is checked as if `nodes` is never `null`. A later body edit that Flow accepts under the narrower type (e.g. `nodes !== undefined ? [nodes] : undefined`) would yield `[null]` and crash on `node.loc` in the reduce at line 119. Line 94 should be widened too.
Fix: —

### Item 3
Location: src/error/GraphQLError.js:25
Claim: The type `nodes?: ... | void | null` is spelled differently from every sibling parameter, which uses the `?Type` maybe form.
Consequence: Maintenance cost only: `| void` is already implied by `?:`, and `| void | null` is just `?(...)` written by hand. `nodes?: ?($ReadOnlyArray<ASTNode> | ASTNode)` would match `source?: ?Source` and the rest, and could be applied identically to line 94.
Fix: —

### Item 4
Location: src/error/__tests__/GraphQLError-test.js:22
Claim: The fixture parse and `invariant` calls were moved to module top level, so they run at file load rather than inside a test or `before` hook.
Consequence: If `parse` throws or an invariant fails (parser regression, dedent change), the exception is raised while mocha is loading the file. The whole `src/**/__tests__` run aborts with a load error instead of reporting one failing GraphQLError test. `invariant` is also called with no message, so the error text would be 'undefined'.
Fix: —

### Item 5
Location: src/error/__tests__/GraphQLError-test.js:104
Claim: 'serializes to include message and locations' now reuses the shared multi-line `fieldNode`, dropping the only case built from a plain string via `parse('{ field }')`.
Consequence: Every location test in the file now uses the same node at (2:3) from one explicit `Source`. The former single-line (1:3) case, where the Source is created implicitly by `parse(string)`, is no longer exercised. A regression in line-1 column computation or in `node.loc.source` for implicit sources would not be caught here.
Fix: —

### Item 6
Location: src/error/__tests__/GraphQLError-test.js:30
Claim: `new GraphQLError()` was replaced by `new GraphQLError('str')` to satisfy Flow, removing the only test of zero-argument construction.
Consequence: Plain-JS consumers can still call `new GraphQLError()` with no message. A future change that touches `message` (e.g. `message.trim()`) would throw a TypeError for them with no test failing. Keeping the call with a targeted Flow suppression would preserve the coverage.
Fix: —

### Item 7
Location: src/jsutils/__tests__/inspect-test.js:31
Claim: A bare `// $FlowFixMe` with no explanation was added to suppress a Flow error on the `String.raw` tagged template, instead of writing the expectation in a form that type-checks.
Consequence: The suppression hides any Flow error on that line, including a future real one in the `inspect(...)` call or the `expect` chain. The plain literal `'"\\""'` is an equivalent expected value and needs no suppression.
Fix: —

### Item 8
Location: src/error/__tests__/locatedError-test.js:29
Claim: Both error fixtures are annotated `any` to allow ad-hoc property assignment, which turns off type checking for the `locatedError(e, [], [])` calls in a file newly marked `@flow strict`.
Consequence: Because `e` is `any`, Flow will not flag these call sites if the `locatedError` signature changes (argument order, parameter types). The enabled typing gives no protection for two of the three tests in the file. A narrow type such as `Error & { path?: mixed, nodes?: mixed, ... }` would keep the calls checked.
Fix: —

### Item 9
Location: src/error/__tests__/printError-test.js:62
Claim: The new variables `opA`/`opB` hold `ObjectTypeDefinition` nodes, not operations, and the null check on `fieldA`/`fieldB` is deferred to one combined invariant after both documents are parsed.
Consequence: Readability cost: `opA` suggests an OperationDefinition while the invariant on the next line asserts `Kind.OBJECT_TYPE_DEFINITION`. The combined `invariant(fieldA && fieldB)` at line 80 also cannot say which document lacked the field. Naming them `typeA`/`typeB` and asserting each field next to its extraction would be clearer.
Fix: —
