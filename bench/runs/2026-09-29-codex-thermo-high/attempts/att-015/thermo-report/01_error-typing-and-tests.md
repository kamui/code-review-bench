# Error typing and tests

## Scope and measurements

Reviewed the committed changes to `src/error/GraphQLError.js`, `src/error/__tests__/GraphQLError-test.js`, `src/error/__tests__/locatedError-test.js`, `src/error/__tests__/printError-test.js`, and `src/jsutils/__tests__/inspect-test.js`, along with the surrounding GraphQLError implementation and located/printed error helpers. The patch changes five files by 48 insertions and 47 deletions. No changed file approaches the 1,000-line threshold, and the test conversions do not add branching or a new abstraction layer.

Commands used for evidence collection were `git diff --find-renames --unified=80 main...review-head`, `sed` on the changed implementation and its callers, `rg` over Flow configuration/references, and `nl -ba` for the cited line anchors. No test or Flow command was executed; verification is static only.

## Constructor contract

In `src/error/GraphQLError.js:25`, the `declare class GraphQLError` constructor now accepts `null` in its `nodes` union. At `src/error/GraphQLError.js:94`, the exported implementation’s annotated `nodes` parameter remains `$ReadOnlyArray<ASTNode> | ASTNode | void`, with no `null`. The runtime implementation treats null as absence through its falsey fallback at lines 102–108, and this patch’s strict tests intentionally pass null in several calls. Updating only the declared class signature leaves two source-level contracts for the same exported constructor out of sync and makes it unclear whether null is supported at this boundary.

The direct code-judo fix is to make both signatures express the same contract, preferably by aligning the implementation annotation with the declared class. If maintaining both declarations is avoidable in this code pattern, collapse them into one authoritative type boundary instead of carrying a duplicated signature. This review did not run Flow, so it does not claim which signature Flow selects at each external call site; the mismatch itself is directly visible in the changed source.

**Finding:** In `src/error/GraphQLError.js`, the declared constructor now accepts `null` for `nodes`, while the exported implementation signature still excludes it. This leaves the public typing dependent on which signature a Flow consumer sees and makes the implementation’s contract contradict the declaration. Add `null` to the implementation parameter union too, or consolidate the signatures so there is one authoritative constructor contract.

**Anchor:** `src/error/GraphQLError.js`, lines 25 and 94.

**Verification status:** Not typechecked in this review. Static evidence only.

## Fallback test

At `src/error/__tests__/GraphQLError-test.js:57–63`, the test still says “creates new stack if original error has no stack,” but the setup changed from a stackless `{ message: 'original' }` object to `new Error('original')`. In the ordinary test runtime, `new Error` has a stack. The implementation reuses `originalError.stack` when truthy and reaches its generated-stack branches only when that stack is absent (`src/error/GraphQLError.js:189–199`). Consequently, the revised assertion that `e.stack` is a string can pass while the fallback is never exercised. This loses regression coverage in the same change that tightens `originalError` to `Error`.

Keep the argument as a real `Error` for Flow, but remove or clear its stack before passing it to `GraphQLError`, then assert that the result has a stack and that the original error remains attached. This preserves strict typing while testing the behavior the test name promises; it is simpler than retaining an untyped object or weakening the parameter type.

**Finding:** In `src/error/__tests__/GraphQLError-test.js`, the test named “creates new stack if original error has no stack” now constructs an ordinary `Error`, which normally has a stack. As written, the test exercises reuse of `original.stack`, so it no longer protects the fallback branch it names. Keep the value typed as `Error` but explicitly clear its stack before constructing `GraphQLError`, then assert the generated stack is present.

**Anchor:** `src/error/__tests__/GraphQLError-test.js`, lines 57–63.

**Verification status:** Not executed. The conclusion follows from the changed test setup and the branch condition in the implementation.

## Other changes reviewed

The strict test conversions use project-local `invariant` checks to narrow parsed AST nodes and introduce `any` only where tests intentionally add GraphQL-shaped properties to ordinary errors. The `inspect` test adds a local `$FlowFixMe` for the quoted-string case. These additions are narrow and do not create a material maintainability problem in this diff. The source dedenting and shared AST setup in `GraphQLError-test.js` are straightforward and avoid repeated parsing. No additional code-judo restructuring is justified by these small, focused changes.
