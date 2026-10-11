# Flow contracts in GraphQLError and locatedError tests

## Scope and measurements

Reviewed the five-file committed diff (`main...review-head`): 48 insertions and 47 deletions. The changed production code is a single Flow annotation in `src/error/GraphQLError.js`; other changes enable strict Flow in three error tests and one inspect test, with shared AST setup moved to file scope in `GraphQLError-test.js`.

The diff does not push any file across 1,000 lines and adds no branching or runtime behavior. Moving the common parsed source and nodes to one fixture removes duplicated parsing and keeps the position assertions aligned.

## Finding 1 — Keep the nullable `nodes` contract consistent

`src/error/GraphQLError.js:25` adds `null` to the declared class constructor’s `nodes` parameter, while the exported implementation’s parameter at line 94 remains `$ReadOnlyArray<ASTNode> | ASTNode | void`. The implementation then normalizes `nodes` using `Array.isArray(nodes)` and truthiness, so `null` is deliberately accepted at runtime as “no nodes”; the tests also call it that way. Leaving the two signatures different makes the boundary unclear: construction through the declared class accepts an input the function annotation rejects.

**Worked code-judo proposal:** introduce a local Flow alias for the accepted node input, for example `$ReadOnlyArray<ASTNode> | ASTNode | void | null`, then use that alias in both signatures. This removes a second independently maintained description of the same contract while preserving the current runtime behavior. If null should instead be rejected, change the callers and runtime contract together; the current falsy normalization and existing calls indicate that null is intentional.

**Action:** update the exported function parameter annotation to match the declared constructor, preferably through a shared alias, and verify both typechecking and the nullable call sites.

## Finding 2 — Type the `locatedError` fixtures instead of opting out

`src/error/__tests__/locatedError-test.js:29` and `:41` newly annotate test errors as `any`. In the first case, the test mutates five extra GraphQL-shaped properties and a name; in the second, it assigns a string path. Those are exactly the structural distinctions the tests exercise, but `any` removes checking of the fixture assignments and of `locatedError(e, [], [])` for both cases. Enabling `@flow strict` around these lines therefore leaves the central values of these tests outside the type contract.

**Worked code-judo proposal:** give the two fixtures explicit shapes that intersect `Error` with only the properties each test uses, and construct each with `Object.assign(new Error(...), {...})` or a small local typed factory. Keep the first shape’s empty locations/nodes/positions and null source distinct from the second shape’s string path. This preserves the test’s intent while making the test data itself the source of its contract, without a general-purpose abstraction.

**Action:** replace both `any` annotations with explicit Flow object types and confirm that the test calls remain accepted by the `locatedError` API.

## Verification status

The allowed focused runtime command was run once for all four changed test files:

`./node_modules/.bin/mocha --require @babel/register --require @babel/polyfill src/error/__tests__/GraphQLError-test.js src/error/__tests__/locatedError-test.js src/error/__tests__/printError-test.js src/jsutils/__tests__/inspect-test.js`

Result: **27 passing**. The command emitted an existing outdated Browserslist data warning. Flow checking was not run. `git diff --check main...review-head` completed without whitespace errors. The review did not modify the clone.
