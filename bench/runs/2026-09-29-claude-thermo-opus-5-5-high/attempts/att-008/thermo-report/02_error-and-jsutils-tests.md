# 02 — Test files converted to `@flow strict`

Scope: `src/error/__tests__/GraphQLError-test.js`, `src/error/__tests__/locatedError-test.js`,
`src/error/__tests__/printError-test.js`, `src/jsutils/__tests__/inspect-test.js`.

Commands used for this subsystem (run from the clone root, nothing written into the clone):

```
git diff main...review-head
./node_modules/.bin/mocha --require @babel/register --require @babel/polyfill \
  src/error/__tests__/*-test.js src/jsutils/__tests__/inspect-test.js
    -> 27 passing
NODE_PATH=$PWD/node_modules node ../clone-work/scratch/stack-check.js
git status --porcelain   -> (empty; clone unchanged)
```

## Finding 2.1 — "creates new stack if original error has no stack" no longer tests that case

`src/error/__tests__/GraphQLError-test.js:57-64`. To satisfy Flow's `?Error` parameter type, the
PR replaced the fixture `const original = { message: 'original' };` with
`const original = new Error('original');`. A real `Error` always has a `stack` in V8, so the
constructor now takes the `originalError && originalError.stack` branch at
`src/error/GraphQLError.js:195-200` — exactly the branch already covered by the preceding test
"uses the stack of an original error" (lines 41-55). The fallback path this test is named for
(original error present but stackless → `Error.captureStackTrace` at line 201-202) is no longer
exercised by any test with an `originalError`, and the assertions (`e.stack` is a string) are too
weak to notice: they pass either way.

Verified with a scratch script (`clone-work/scratch/stack-check.js`) that loads the real
`src/error/GraphQLError.js` through the clone's babel:

```
PR fixture: original has stack? string | e.stack === original.stack? true
delete-stack fixture: o2.stack undefined | e2.stack string? string | ...
base fixture: e3.stack first line msg
```

The first line shows the PR's fixture produces `e.stack === original.stack`, i.e. the test is now
a duplicate of its neighbor. This is a quiet coverage regression introduced in the name of type
cleanliness: the type annotation was satisfied by changing what the test tests.

**Remedy (worked).** Keep the fixture a real `Error` (so Flow is satisfied without `any`) but make
it genuinely stackless, and tighten the assertion so the test fails if the wrong branch is taken:

```js
it('creates new stack if original error has no stack', () => {
  const original = new Error('original');
  delete original.stack; // V8 errors carry an own, configurable `stack`
  const e = new GraphQLError('msg', null, null, null, null, original);
  expect(e.name).to.equal('GraphQLError');
  expect(e.stack).to.be.a('string');
  expect(e.stack).to.not.equal(original.stack);
  expect(e.message).to.equal('msg');
  expect(e.originalError).to.equal(original);
});
```

The scratch script's second line confirms that `delete` leaves `original.stack === undefined` and
that `GraphQLError` then synthesizes its own string stack. If Flow objects to `delete` on the
`stack: string` field of its `Error` lib def, the honest fallback is a narrowly-scoped
`const original: any = { message: 'original' };` — the same `any` idiom this PR already uses in
`locatedError-test.js` — rather than silently changing the scenario. Flow was not run, so which
of the two spellings Flow accepts is unverified; the runtime behavior of both is verified.

## Finding 2.2 — `printError-test` copy-pastes a mis-named extraction block and adds guard noise

`src/error/__tests__/printError-test.js:62-80`. The test needs "the type node of the first field
of the first definition" from two documents. The PR writes the same three-line
extract-refine-index sequence twice, names the results `opA`/`opB` even though the `invariant` on
the very next line asserts they are `OBJECT_TYPE_DEFINITION`s (not operations), and then adds a
separate `invariant(fieldA && fieldB)` on line 80. The pre-PR version was one line per document
(`sourceA.definitions[0].fields[0].type`); the Flow conversion roughly quadrupled it and introduced
a misleading name.

Part of the added guarding is redundant. Flow types array index reads (`arr[0]`) as `T`, not
`?T`, so `invariant(fieldA && fieldB)` on line 80 refines nothing, and neither do the `opA &&` /
`opB &&` conjuncts on lines 63 and 77. The same pattern appears in the shared fixture at
`src/error/__tests__/GraphQLError-test.js:24-26`: `operationNode &&` in the first `invariant` and
the entire `invariant(fieldNode)` on line 26 do no type work (`fieldNode` is only ever passed
where `ASTNode` is accepted). Guards that do nothing teach the next reader that they are needed.

**Remedy (worked).** Name the concept once and use it twice:

```js
function firstFieldType(doc) {
  const typeDef = doc.definitions[0];
  invariant(typeDef.kind === Kind.OBJECT_TYPE_DEFINITION && typeDef.fields);
  return typeDef.fields[0].type;
}

const fieldTypeA = firstFieldType(parse(new Source(dedent`…`, 'SourceA')));
const fieldTypeB = firstFieldType(parse(new Source(dedent`…`, 'SourceB')));
const error = new GraphQLError('Example error with two nodes', [fieldTypeA, fieldTypeB]);
```

This keeps the only refinements Flow actually needs (the `kind` check to reach the
`ObjectTypeDefinitionNode` member of the union, and `fields` because it is declared optional —
`+fields?:` at `src/language/ast.js:436`), restores the original variable names, and deletes
the duplicated block and the no-op `invariant`. In `GraphQLError-test.js`, drop line 26 and the
`operationNode &&` conjunct on line 24.

**Verification status.** Duplication and naming verified by reading the head file. The
redundancy claim rests on Flow's documented (unsound) array-access typing in 0.86; Flow was not
executed in this run, so it is a reasoned rather than type-checked claim.

## Finding 2.3 (minor) — Bare `$FlowFixMe` where a plain literal would do

`src/jsutils/__tests__/inspect-test.js:31-32`. The PR suppresses Flow on the whole assertion line
`expect(inspect('"')).to.equal(String.raw\`"\""\`);` with an unexplained `// $FlowFixMe`. The
suppression exists only because Flow's lib def for `String.raw` rejects tagged-template use; the
`String.raw` itself exists only to avoid writing a backslash escape. Writing the expected value as
an ordinary literal, `expect(inspect('"')).to.equal('"\\""');`, removes both the tag and the
suppression, and stops the suppression from also masking any future type error in the `inspect`
call on the same line.

There is precedent for a bare `// $FlowFixMe` before `String.raw` in
`src/utilities/__tests__/schemaPrinter-test.js:178`, which is why this is minor; but there the
raw string is a multi-line SDL block where `String.raw` earns its keep, whereas here it wraps four
characters. If the suppression is kept, it should carry a reason (as `src/jsutils/objectValues.js:15`
and `isInteger.js:14` do) so it can be removed when Flow's lib def is fixed.

**Verification status.** Verified by reading the file and grepping `$FlowFixMe`/`String.raw`
usage across `src/`. The literal `'"\\""'` is the same four-character string `"\""` as
`String.raw\`"\""\``.

## Considered and not raised

- **Shared module-level fixture in `GraphQLError-test.js` (lines 17-26).** This is a good
  change: four tests previously re-parsed an identical ad-hoc indented source; they now share one
  `dedent`ed fixture, and the expected positions/columns were updated consistently (`8→4`,
  `7→3`, `10→6`, `9→5`; the serialize test's line moved `1→2` because it now uses the shared
  multi-line fixture). The `loc.start === 0` test still holds because the dedented source starts
  with `{`. All 27 tests pass.
- **`const e: any` in `locatedError-test.js:29,41`.** These tests deliberately construct
  GraphQLError-*ish* and elasticsearch-*ish* objects that do not fit `Error`'s type; a local `any`
  is the direct, honest expression of that intent. Not raised.
- **`new GraphQLError('str')` in the subclass test.** Correct: the previous zero-arg call was
  only legal because the file was `@noflow`.
- **`@flow strict` with no `[strict]` section in `.flowconfig`.** The repo's `.flowconfig` has no
  `[strict]` lint list, so `@flow strict` currently behaves like `@flow`. This is a repo-wide
  pre-existing condition shared by the preceding "Enable Flow typings" commits, not something this
  PR introduced; noted for context only.
- File sizes: all touched files are under 225 lines.
