# Detail 01: src/error typing change and Flow-enabled tests

Scope: `git diff main...review-head` (5 files, +48/−47, one commit). No file is near 1000 lines, so the file-size rule does not apply.

## Measurements and commands

- `git diff main...review-head` was read in full.
- `./node_modules/.bin/mocha --require @babel/register --require @babel/polyfill src/error/__tests__/*-test.js src/jsutils/__tests__/inspect-test.js` gave 27 passing.
- `./node_modules/.bin/flow check` gave "Found 0 errors".
- `grep -n nodes src/error/GraphQLError.js` shows the two `nodes` signatures at lines 25 and 94.

## Finding 1: the constructor contract is now stated twice and the two copies disagree

Status: CONFIRMED by reading the file. Flow does not flag it.

`src/error/GraphQLError.js:25` adds `| null` to the `nodes` parameter of the `declare class GraphQLError` constructor. The real implementation, `export function GraphQLError(...)` at `src/error/GraphQLError.js:94`, still declares `nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void`.

The body already handles null. `Array.isArray(nodes) ? ... : nodes ? [nodes] : undefined` treats null the same as undefined. The implementation therefore accepts null at runtime while its own annotation forbids it. Flow only checks callers against the `declare class`, so the mismatch is invisible. The next person to touch either signature has no signal that the two must agree.

The same drift already exists for `originalError`. It is `?Error` in the declaration and `?Error & { +extensions: mixed }` in the implementation. This PR did not cause that, but it adds a second divergence in the same block.

Code-judo proposal: keep a single source of truth. Update line 94 to `| void | null` in this same change. Better, use `?$ReadOnlyArray<ASTNode> | ASTNode` in both places, which is the idiom every neighbouring parameter already uses (`?Source`, `?$ReadOnlyArray<number>`, and so on). That also removes the odd `void | null` pair the PR introduces. The author's own review comment says every other argument accepts null, so the pair is inconsistent. The pair is a smell in its own right, because `nodes?: X | void | null` says "optional, or undefined, or null", which is what `?X` already means.

Also consider `+nodes: $ReadOnlyArray<ASTNode> | void` at line 65. The tests now pass null in, and the property normalizes it to undefined. That is fine, but it is worth a one-line doc note.

## Finding 2: `: any` and `$FlowFixMe` are used to silence Flow in tests

Status: CONFIRMED.

- `src/error/__tests__/locatedError-test.js:29` and `:41` change `const e = new Error(...)` to `const e: any = new Error(...)` so that `e.locations`, `e.path` and similar fields can be attached. Both tests exist to show that hand-built error-like objects behave correctly. A typed local shape would say this directly, for example `Object.assign(new Error('...'), { locations: [], path: [], ... })`, or a single small helper. With `any`, the whole variable, including the later `locatedError(e, ...)` call, loses checking.
- `src/jsutils/__tests__/inspect-test.js:31` adds a bare `// $FlowFixMe` above `inspect('"')` with no explanation of what Flow rejects. It is under an unrelated `@flow strict` upgrade, so the suppression looks like a workaround for a `String.raw` typing gap. It should say so, or use a plain string literal such as `'"\\""'` and avoid the suppression altogether.

## Finding 3: repeated invariant narrowing boilerplate in printError-test and GraphQLError-test

Status: CONFIRMED. This is a legibility and duplication concern, not a correctness one.

`src/error/__tests__/printError-test.js:62-64` and `:76-78` are identical except for the A/B suffix. Each is a `parse(...)`, an `invariant(x && x.kind === Kind.OBJECT_TYPE_DEFINITION && x.fields)`, and an index. A further `invariant(fieldA && fieldB)` follows at line 80. The variable names `opA` and `opB` are misleading. They hold an `ObjectTypeDefinition`, not an operation. The old code got the same result with a single expression per side.

The reframing is a local helper, for example `getFieldTypeNode(source)`. It could parse, narrow once, and return `fields[0].type`. The test body would then keep its old shape: two calls and one `new GraphQLError('...', [typeA, typeB])`. The two-line invariant chain and both renames disappear. The renames `sourceA`→`docA` and `sourceB`→`docB` are churn that exists only to make room for the narrowing.

`src/error/__tests__/GraphQLError-test.js:14-26` hoists a shared `source`/`ast`/`operationNode`/`fieldNode` fixture to module scope, with `invariant` calls running at import time. This removes three copies of the parse block, which is good. However:

- A failing invariant now breaks the whole file at load time rather than one test.
- The assertions became magic numbers tied to the dedent layout (`positions [4]`, `[6]`, `{line: 2, column: 3}`, `column: 5`). Only the `[6]` test case has any relationship to the source text that is visible in the test. Deriving them from `source.body.indexOf('field')` would keep them honest.

## Finding 4: minor test churn

- `src/error/__tests__/GraphQLError-test.js:20-21` changes `new GraphQLError()` to `new GraphQLError('str')`, and the `original` at `:58` becomes a real `Error`. Both are legitimate consequences of the typing. Note that the second one changes what the test is exercising. The test's title is "creates new stack if original error has no stack", but a real `Error` now always has a stack, so the branch the test names may no longer be the one it hits. Behaviour coverage may have quietly weakened. This should be checked, and the test should construct an error whose `stack` is absent (`delete original.stack` or `original.stack = undefined`) so the intended path is still covered.

## Verdict for this subsystem

The change works, tests and Flow pass, and it is a reasonable typing migration. Approval is withheld because of Finding 1, where the constructor type has been duplicated and left inconsistent, and Finding 4, where a test may no longer exercise the case in its title. Findings 2 and 3 are cleanups.
