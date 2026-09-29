# Thermo-nuclear code quality review: graphql-js #1582

Range: `5384d21..7e39a12` (`git diff main...review-head`). One commit, 5 files, +48/−47.
Change: enable `@flow strict` on four error and inspect tests, and widen the `GraphQLError` constructor `nodes` type to accept `null`.
Evidence run: mocha on the touched tests gave 27 passing. `flow check` reported 0 errors.
Detail file: `01_error-typing-and-tests.md`.

## Verdict

Not a structural regression. No file approaches 1000 lines, no branching was added, and the test hoisting removes some duplication. The bar of "no missed cleanup" is not met, though. There is one real contract problem in production code and one test that may have stopped covering the case it names. The rest is boilerplate that a small helper would remove.

## Findings

**Finding 1: the constructor type is now declared twice and the copies disagree (`src/error/GraphQLError.js:25` vs `:94`).** The PR adds `| null` to `nodes` in the `declare class GraphQLError` constructor, but the real `export function GraphQLError(...)` signature is unchanged. Flow only checks callers against the declaration, so the divergence is invisible to the tooling. The body already treats null like undefined. The implementation therefore accepts a value its own annotation forbids. The `originalError` type already disagrees between the two in the same way. The remedy is to update line 94 in the same change, and better to write `nodes?: ?$ReadOnlyArray<ASTNode> | ASTNode`, the idiom used by every neighbouring parameter. That replaces the redundant `void | null` pair with the `?X` form and lets the two signatures be compared at a glance. Evidence is in detail section "Finding 1".

**Finding 2: a test may no longer cover its titled case (`src/error/__tests__/GraphQLError-test.js:58`).** "creates new stack if original error has no stack" used to pass a plain object `{ message: 'original' }`. It now passes `new Error('original')`, which always has a stack. Flow accepts it, but the test no longer exercises the "no stack" branch it is named for. The remedy is to build the original error and remove its stack (`original.stack = undefined`), or to type the plain object explicitly, so that the intended path stays covered. Evidence is in detail section "Finding 4".

**Finding 3: `any` and an unexplained `$FlowFixMe` silence Flow instead of modelling the input (`src/error/__tests__/locatedError-test.js:29,41`, `src/jsutils/__tests__/inspect-test.js:31`).** The `locatedError` tests are about hand-built error-like objects, and typing those as `any` removes checking for the whole variable. Use `Object.assign(new Error(...), {...})` or a small helper so the shape is explicit. The bare `// $FlowFixMe` on `inspect('"')` has no explanation, and probably compensates for a `String.raw` typing gap. Either say so in a comment or use a plain string literal, which avoids the suppression. Evidence is in detail section "Finding 2".

**Finding 4: duplicated narrowing boilerplate and misleading names in `printError-test.js`, plus import-time invariants and magic offsets in `GraphQLError-test.js` (`printError-test.js:62-80`, `GraphQLError-test.js:14-26`).** The A and B blocks in `printError-test.js` are identical apart from their suffix, and `opA`/`opB` actually hold object type definitions. A small helper such as `getFieldTypeNode(source)` would put the parse, the narrowing and the `fields[0].type` lookup in one place. The two renames `sourceA→docA` and `sourceB→docB` would then be unnecessary, and the test body would stay close to its original two-node shape. In `GraphQLError-test.js` the module-level fixture removes three copied parse blocks, which is an improvement. But an invariant failure there now breaks the whole file at load. The expected offsets (`4`, `6`, column `3`, column `5`) are hard-coded against the dedent layout and would be safer derived from `source.body`. Evidence is in detail section "Finding 3".

## Proposed remediation sequence

1. Fix the `nodes` signature at `GraphQLError.js:94` and use the `?X` form in both signatures. This is the only production-code change and takes a few lines.
2. Restore the no-stack case in the `GraphQLError-test.js` test so its name is true again.
3. Replace the `any` locals and the bare `$FlowFixMe` with typed shapes or a literal.
4. Extract the field-type helper in `printError-test.js`. Optionally derive offsets from `source.body`.

## Questions

None.
