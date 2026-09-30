# Review blind-7f90e2

### Item 1
Location: src/error/__tests__/GraphQLError-test.js:57-64
Claim: In `src/error/__tests__/GraphQLError-test.js:57-64`, the fixture for
`creates new stack if original error has no stack` was changed from `{ message: 'original' }` to
`new Error('original')` so that it type-checks. A real `Error` always carries a `stack`, so the
constructor now takes the `originalError.stack` branch and reuses the original stack. That is
exactly what the preceding test `uses the stack of an original error` (lines 41-55) already
covers. A scratch probe that runs the constructor confirmed `e.stack === original.stack` for the
new fixture. The assertions are weak enough (`e.stack` is "a string") that the test still passes,
so the lost coverage is invisible in CI. The fix is to keep the typed `Error` but set
`original.stack = ''`, which a second probe confirmed forces the fresh-stack path, and to assert
`e.stack` is not equal to `original.stack`. The alternative is an explicit
`({ message: 'original' }: any)` fixture. **Confirmed by execution.** Details and the worked test
body are in `01_error-contract-and-tests.md`, Finding 1.1.
Consequence: —
Fix: —

### Item 2
Location: src/error/GraphQLError.js:25-94
Claim: `src/error/GraphQLError.js:25` now declares
`nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void | null`, but the implementation function at
`src/error/GraphQLError.js:94` still says `nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void`.
This file keeps two parallel constructor signatures in sync by hand, and they already disagree on
`originalError` (lines 29 vs. 98). This PR adds a second disagreement to a parameter it explicitly
set out to fix. The runtime is fine, because falsy `nodes` already becomes `undefined`. The
maintainability cost is that readers cannot tell which list is the contract, and the next edit has
two places to update and no warning. The code-judo fix is to write the parameter the same way as
its siblings, `nodes?: ?($ReadOnlyArray<ASTNode> | ASTNode)`, in **both** places, and ideally to
align `originalError` at the same time or collapse the duplication behind one shared parameter
type. **Mismatch confirmed by reading; Flow's current tolerance of it is inferred, not type-checked.**
See `01_error-contract-and-tests.md`, Finding 1.2.
Consequence: —
Fix: —

### Item 3
Location: src/error/__tests__/printError-test.js:62-80
Claim: `src/error/__tests__/printError-test.js:62-80` repeats the same "take the first definition, check
its kind, take `fields[0]`" block for two sources. It names the results `opA`/`opB` even though
the invariant checks `Kind.OBJECT_TYPE_DEFINITION`, and it ends with `invariant(fieldA && fieldB)`,
which narrows nothing because Flow types array index reads as non-optional. The same redundancy
appears in `src/error/__tests__/GraphQLError-test.js:24-26` (`operationNode &&` and
`invariant(fieldNode)`), where the invariants now also run at module load time. A single local
`firstFieldType(doc)` helper would halve the `printError` test body and remove the misleading
names. **Duplication and naming confirmed by reading; the redundancy is inferred from Flow's typing
rules.** See `01_error-contract-and-tests.md`, Finding 1.3.
Consequence: —
Fix: —

### Item 4
Location: src/jsutils/__tests__/inspect-test.js:31-32
Claim: `src/jsutils/__tests__/inspect-test.js:31` adds an unexplained `// $FlowFixMe` only because Flow
0.86 cannot type ``String.raw`"\""` ``. The bare form suppresses every error on the line, including
errors in the `inspect(...)` call under test, and that works against the PR's purpose of enabling
type-checking. There is repo precedent for the same pattern (`schemaPrinter-test.js:178`), so this
is low severity. Still, the plain literal `'"\\""'` expresses the same four characters with no
suppression. **Confirmed by reading.** See `02_jsutils-inspect-test.md`, Finding 2.1.
Consequence: —
Fix: —
