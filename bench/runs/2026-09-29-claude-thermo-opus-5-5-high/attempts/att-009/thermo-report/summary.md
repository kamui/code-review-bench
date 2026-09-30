# Thermo-nuclear code quality review — graphql/graphql-js#1582

**PR:** "Enable Flow typings on errors tests + Fix typing for Error constructor"
**Range:** `5384d218539dbb6bb39b25e0b7a5dcdd69ad8a11..7e39a122eea9292eeffa6905ffdf8a60c5161cfd`
(5 files, +48/−47, 1 commit)

## Verdict

**Changes requested — small, but one finding is a real regression.** The PR has a good goal:
turning `@noflow` test files into `@flow strict` and fixing the `GraphQLError` constructor type
that the stricter checking exposed. Most of the churn is fine. Hoisting one dedented `Source`
fixture in `GraphQLError-test.js` removes three copies of the same inline query, and that is an
improvement. There are no file-size, layering, or spaghetti-branching concerns, because nothing
here touches production control flow.

The main problem is that one Flow-driven fixture change quietly changed what a test checks. The
test named "creates new stack if original error has no stack" now passes an original error that
*has* a stack. It has become a duplicate of the test before it, and the fallback branch it was
written for has no coverage left. Separately, the constructor type fix was applied to only one of
the two hand-maintained signatures in `GraphQLError.js`, so the declared contract and the
implementation drift further apart. The test-side narrowing boilerplate and one blanket
`$FlowFixMe` are minor cleanups.

All four changed test files pass under mocha (27 passing). Flow was not run: the execution
allowance covers focused mocha runs only, so type-level claims below are inferred from reading
the code.

## Findings

### 1. The "no stack" `GraphQLError` test now checks the "has stack" branch (test-coverage regression)

In `src/error/__tests__/GraphQLError-test.js:57-64`, the fixture for
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

### 2. `nodes` was widened in the `declare class` signature but not in the implementation signature

`src/error/GraphQLError.js:25` now declares
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

### 3. The test narrowing boilerplate is duplicated, partly redundant, and misnamed

`src/error/__tests__/printError-test.js:62-80` repeats the same "take the first definition, check
its kind, take `fields[0]`" block for two sources. It names the results `opA`/`opB` even though
the invariant checks `Kind.OBJECT_TYPE_DEFINITION`, and it ends with `invariant(fieldA && fieldB)`,
which narrows nothing because Flow types array index reads as non-optional. The same redundancy
appears in `src/error/__tests__/GraphQLError-test.js:24-26` (`operationNode &&` and
`invariant(fieldNode)`), where the invariants now also run at module load time. A single local
`firstFieldType(doc)` helper would halve the `printError` test body and remove the misleading
names. **Duplication and naming confirmed by reading; the redundancy is inferred from Flow's typing
rules.** See `01_error-contract-and-tests.md`, Finding 1.3.

### 4. A blanket `$FlowFixMe` in `inspect-test.js` where the construct could simply be deleted

`src/jsutils/__tests__/inspect-test.js:31` adds an unexplained `// $FlowFixMe` only because Flow
0.86 cannot type ``String.raw`"\""` ``. The bare form suppresses every error on the line, including
errors in the `inspect(...)` call under test, and that works against the PR's purpose of enabling
type-checking. There is repo precedent for the same pattern (`schemaPrinter-test.js:178`), so this
is low severity. Still, the plain literal `'"\\""'` expresses the same four characters with no
suppression. **Confirmed by reading.** See `02_jsutils-inspect-test.md`, Finding 2.1.

## Checked and accepted

- `locatedError-test.js` uses `const e: any = new Error(...)` to build deliberately off-contract
  "GraphQLError-ish" objects. That is an honest boundary for these tests.
- The shared `dedent` fixture in `GraphQLError-test.js` and the recomputed positions and locations
  are consistent and pass.
- No file crosses or approaches 1000 lines, and no production control flow changed.

## Proposed remediation sequence

1. Restore the "no stack" coverage (Finding 1): set `original.stack = ''` and assert the resulting
   stack differs from the original.
2. Make the two `GraphQLError` constructor signatures agree (Finding 2): use
   `nodes?: ?($ReadOnlyArray<ASTNode> | ASTNode)` in both, and align `originalError` while there.
3. Tidy the test narrowing (Finding 3): add a `firstFieldType` helper in `printError-test.js`,
   rename away from `op*`, and drop the redundant invariants.
4. Replace the `String.raw` + `$FlowFixMe` pair with a plain literal (Finding 4).

## Detail files

- `01_error-contract-and-tests.md`: `GraphQLError.js` signature drift, the stack-branch coverage
  regression with probe output, and the test narrowing cleanup.
- `02_jsutils-inspect-test.md`: the `$FlowFixMe` suppression in `inspect-test.js`.
