# Detail 01: `src/error/GraphQLError.js` constructor declaration

## Change

`git diff main...review-head -- src/error/GraphQLError.js` shows a single line: `nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void` becomes `nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void | null`.

## Evidence

Reading lines 22 to 33 of the declaration shows six other parameters after `message`, and every one of them uses the `?T` maybe form (`source?: ?Source`, `positions?: ?$ReadOnlyArray<number>`, `path?: ?$ReadOnlyArray<string | number>`, `originalError?: ?Error`, `extensions?: ?{ [key: string]: mixed }`). `nodes` is the only one spelled as an explicit union with `void`. Tests at `GraphQLError-test.js` lines 59, 91, 111, 124 and 139 pass `null` for `nodes`, which is why the widening was needed to enable strict Flow.

## Verification

`./node_modules/.bin/flow check` reported 0 errors at the head. The mocha run over `src/error/__tests__/*-test.js` and `src/jsutils/__tests__/inspect-test.js` reported 27 passing. I did not run Flow on the proposed rewrite because the clone is read-only; `?(A | B)` is by definition `A | B | void | null`, so the equivalence follows from Flow's semantics.

## Worked proposal

Replace the line with `nodes?: ?($ReadOnlyArray<ASTNode> | ASTNode),`. This is a one-line change, removes the only non-uniform parameter, and needs no test changes. Status: unverified by execution, equivalent by definition.
