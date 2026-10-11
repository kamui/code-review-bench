# Error constructor type contract

## Finding

**`src/error/GraphQLError.js:25,94` — Keep the nullability contract aligned.** The declaration now permits `null` for `nodes`, while the exported function signature still permits only arrays, AST nodes, or `void`. The implementation already normalizes falsey `nodes`, so the public type and implementation contract should agree. Add `null` to the implementation signature as well, or remove the duplicated signature source if the Flow structure allows it.

## Evidence

The changed declaration at line 25 is:

```js
nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void | null,
```

The exported function at line 94 remains:

```js
nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void,
```

The implementation's `_nodes` normalization treats non-array falsey values as absent (`src/error/GraphQLError.js:102-108`). The strict tests also construct errors with `null` in the second argument, including the source-and-positions case (`src/error/__tests__/GraphQLError-test.js:91`). Thus null is part of the exercised runtime contract and is now in the class declaration, but not the function implementation signature.

This is a duplicated type contract in one module. The new annotation addresses the test-facing declaration without keeping the implementation's own parameter type in sync, leaving two descriptions of the same constructor boundary to drift independently.

## Worked code-judo proposal

Make the two constructor signatures agree by adding `null` to the implementation parameter union. If this Flow pattern permits a single signature to describe both the declared class and exported implementation, consolidate the duplicated parameter contract instead. The former is a narrowly scoped change; the latter removes the source of drift altogether. In either case, preserve the existing falsey normalization so runtime behavior remains unchanged.

## Verification status

The focused Mocha command was:

```text
./node_modules/.bin/mocha --require @babel/register --require @babel/polyfill src/error/__tests__/GraphQLError-test.js src/error/__tests__/locatedError-test.js src/error/__tests__/printError-test.js src/jsutils/__tests__/inspect-test.js
```

Result: 27 passing. `git diff --check` was clean. Flow validation was not run under the supplied execution allowance; strict Flow success is therefore unverified.
