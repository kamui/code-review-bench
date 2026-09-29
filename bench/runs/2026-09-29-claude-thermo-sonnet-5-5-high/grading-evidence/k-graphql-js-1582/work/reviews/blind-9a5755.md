# Review blind-9a5755

### Item 1
Location: src/error/GraphQLError.js:25-94
Claim: **Finding 1: the constructor type is now declared twice and the copies disagree (`src/error/GraphQLError.js:25` vs `:94`).** The PR adds `| null` to `nodes` in the `declare class GraphQLError` constructor, but the real `export function GraphQLError(...)` signature is unchanged. Flow only checks callers against the declaration, so the divergence is invisible to the tooling. The body already treats null like undefined. The implementation therefore accepts a value its own annotation forbids. The `originalError` type already disagrees between the two in the same way. The remedy is to update line 94 in the same change, and better to write `nodes?: ?$ReadOnlyArray<ASTNode> | ASTNode`, the idiom used by every neighbouring parameter. That replaces the redundant `void | null` pair with the `?X` form and lets the two signatures be compared at a glance. Evidence is in detail section "Finding 1".
Consequence: —
Fix: —

### Item 2
Location: src/error/__tests__/GraphQLError-test.js:58
Claim: **Finding 2: a test may no longer cover its titled case (`src/error/__tests__/GraphQLError-test.js:58`).** "creates new stack if original error has no stack" used to pass a plain object `{ message: 'original' }`. It now passes `new Error('original')`, which always has a stack. Flow accepts it, but the test no longer exercises the "no stack" branch it is named for. The remedy is to build the original error and remove its stack (`original.stack = undefined`), or to type the plain object explicitly, so that the intended path stays covered. Evidence is in detail section "Finding 4".
Consequence: —
Fix: —

### Item 3
Location: src/error/__tests__/locatedError-test.js:29-41
Claim: **Finding 3: `any` and an unexplained `$FlowFixMe` silence Flow instead of modelling the input (`src/error/__tests__/locatedError-test.js:29,41`, `src/jsutils/__tests__/inspect-test.js:31`).** The `locatedError` tests are about hand-built error-like objects, and typing those as `any` removes checking for the whole variable. Use `Object.assign(new Error(...), {...})` or a small helper so the shape is explicit. The bare `// $FlowFixMe` on `inspect('"')` has no explanation, and probably compensates for a `String.raw` typing gap. Either say so in a comment or use a plain string literal, which avoids the suppression. Evidence is in detail section "Finding 2".
Consequence: —
Fix: —

### Item 4
Location: src/error/__tests__/printError-test.js:62-80
Claim: **Finding 4: duplicated narrowing boilerplate and misleading names in `printError-test.js`, plus import-time invariants and magic offsets in `GraphQLError-test.js` (`printError-test.js:62-80`, `GraphQLError-test.js:14-26`).** The A and B blocks in `printError-test.js` are identical apart from their suffix, and `opA`/`opB` actually hold object type definitions. A small helper such as `getFieldTypeNode(source)` would put the parse, the narrowing and the `fields[0].type` lookup in one place. The two renames `sourceA→docA` and `sourceB→docB` would then be unnecessary, and the test body would stay close to its original two-node shape. In `GraphQLError-test.js` the module-level fixture removes three copied parse blocks, which is an improvement. But an invariant failure there now breaks the whole file at load. The expected offsets (`4`, `6`, column `3`, column `5`) are hard-coded against the dedent layout and would be safer derived from `source.body`. Evidence is in detail section "Finding 3".
Consequence: —
Fix: —
