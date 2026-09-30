# 02 — Flow-enabling the error and jsutils tests

Scope: `src/error/__tests__/GraphQLError-test.js`, `src/error/__tests__/locatedError-test.js`, `src/error/__tests__/printError-test.js`, `src/jsutils/__tests__/inspect-test.js`. Each file switches from `@noflow` to `@flow strict` and adjusts its fixtures to type-check.

Command used for runtime verification (clone's own mocha, offline):

```
./node_modules/.bin/mocha --require @babel/register --require @babel/polyfill \
  src/error/__tests__/GraphQLError-test.js src/error/__tests__/locatedError-test.js \
  src/error/__tests__/printError-test.js src/jsutils/__tests__/inspect-test.js
```

Result: 27 passing. Every finding below survives a green test run. They are about what the tests now fail to test, or how they are written, not about failures.

## Finding 2.1 — "creates new stack if original error has no stack" no longer exercises a stackless original error

**Evidence.** `GraphQLError-test.js:57-64` at head:

```js
  it('creates new stack if original error has no stack', () => {
    const original = new Error('original');
    const e = new GraphQLError('msg', null, null, null, null, original);
    expect(e.name).to.equal('GraphQLError');
    expect(e.stack).to.be.a('string');
    ...
```

At the merge-base, the fixture was `const original = { message: 'original' };`, a plain object with no `stack`. The PR changed it to `new Error('original')` so it satisfies the `?Error` parameter type under `@flow strict`. But a real `Error` always has a string `stack`. So the implementation's first branch at `GraphQLError.js:195` (`if (originalError && originalError.stack)`) is taken, and the stack is copied from the original. The branch the test title describes, where an original error without a stack falls through to `Error.captureStackTrace`, is no longer reached. The only stack assertion is `expect(e.stack).to.be.a('string')`, which passes on either branch, so the test can't tell the difference. It is now effectively a duplicate of the preceding test, "uses the stack of an original error" (lines 41–55), with `null` instead of `undefined` for the middle arguments.

**Why it matters.** This is a quiet coverage regression hidden inside a typing change. The test name still promises something the test no longer checks, and a future change that broke the "original error without stack" path would not be caught by it.

**Remedy.** Keep the fixture a real `Error` (so it type-checks), but blank its stack. Then make the test assert that the stack was freshly created rather than copied:

```js
  it('creates new stack if original error has no stack', () => {
    const original = new Error('original');
    original.stack = '';
    const e = new GraphQLError('msg', null, null, null, null, original);
    expect(e.name).to.equal('GraphQLError');
    expect(e.stack).to.be.a('string').and.not.equal(original.stack);
    expect(e.message).to.equal('msg');
    expect(e.originalError).to.equal(original);
  });
```

An empty string is falsy, so the `originalError.stack` guard fails and control reaches `Error.captureStackTrace`. Unlike `undefined` or `delete`, assigning `''` keeps `stack` a `string` under Flow's `Error` libdef. If the author prefers the original plain-object shape, `const original: any = { message: 'original' };` is also possible. That would match the `any`-typed duck errors this same PR introduces in `locatedError-test.js`.

**Verification status.** Confirmed by execution. I wrote a scratch probe outside the clone (`clone-work/scratch/stack-branch-test.js`) and ran it with the clone's mocha. It shows that with the head fixture `e.stack === original.stack`, i.e. the copy branch is taken. It also shows that with `original.stack = ''` the resulting `e.stack` is a string not equal to the original's, i.e. the fresh-stack branch is taken. Both probe cases passed.

## Finding 2.2 — `printError-test.js` copy-pastes the parse-and-narrow block, names the nodes as if they were operations, and adds a redundant invariant

**Evidence.** `printError-test.js:52-84` at head builds two documents with identical code. For A and for B it does: `parse(new Source(dedent`...`, name))`, take `definitions[0]`, `invariant(x && x.kind === Kind.OBJECT_TYPE_DEFINITION && x.fields)`, take `fields[0]`. It then adds a third `invariant(fieldA && fieldB)` (line 80) before reading `.type` off each.

Three problems stack up here:

- **Duplication.** The narrowing logic, which is the part the PR added, is written out twice. It was a one-liner per document before typing.
- **Misleading names.** The narrowed nodes are named `opA`/`opB`, but they are asserted to be `OBJECT_TYPE_DEFINITION` nodes, not operations. The same PR uses `operationNode` for a real `OPERATION_DEFINITION` in `GraphQLError-test.js`, so a reader skimming both files gets the wrong picture.
- **Redundant invariant.** `invariant(fieldA && fieldB)` adds nothing. Flow types an indexed read of `$ReadOnlyArray<FieldDefinitionNode>` as `FieldDefinitionNode`, not a maybe-type, so no refinement is needed. At runtime, an `undefined` field would throw on `.type` immediately anyway. `invariant(fieldNode)` at `GraphQLError-test.js:26` is redundant for the same reason.

**Remedy (code judo).** The test wants "the type node of the first field of a one-type SDL document, from a named source". Say that once and the duplication, the misnamed temporaries, and the redundant invariant all go away:

```js
  it('prints an error with nodes from different sources', () => {
    function firstFieldTypeNode(body, sourceName) {
      const typeDef = parse(new Source(body, sourceName)).definitions[0];
      invariant(
        typeDef &&
          typeDef.kind === Kind.OBJECT_TYPE_DEFINITION &&
          typeDef.fields,
      );
      return typeDef.fields[0].type;
    }

    const fieldTypeA = firstFieldTypeNode(
      dedent`
        type Foo {
          field: String
        }
      `,
      'SourceA',
    );
    const fieldTypeB = firstFieldTypeNode(
      dedent`
        type Foo {
          field: Int
        }
      `,
      'SourceB',
    );

    const error = new GraphQLError('Example error with two nodes', [
      fieldTypeA,
      fieldTypeB,
    ]);
    ...
```

This also restores the pre-PR names `fieldTypeA`/`fieldTypeB`, so the diff to the assertion block becomes zero. Separately, drop `invariant(fieldNode);` at `GraphQLError-test.js:26`.

**Verification status.** Duplication and naming are confirmed by reading the file at head. The claim that the extra invariants are redundant rests on Flow's documented array-index typing, since Flow was not run (outside the execution allowance). The repo does use single-argument `invariant(x)` as an established test convention (`extendSchema-test.js`, `getOperationRootType-test.js`), so that part is not flagged.

## Finding 2.3 — `inspect-test.js` adds a bare `// $FlowFixMe` to type-check a string literal that doesn't need `String.raw`

**Evidence.** `inspect-test.js:31-32`:

```js
    // $FlowFixMe
    expect(inspect('"')).to.equal(String.raw`"\""`);
```

The suppression has no reason attached, and it suppresses every error on the next line, including any future mistake in the `inspect` call itself. `String.raw` exists here only to avoid writing one escaped backslash.

**Remedy.** Delete the suppression by deleting its cause: write `expect(inspect('"')).to.equal('"\\""');`. That is the same four-character string (`"\""`), with no libdef workaround. If `String.raw` is kept for readability, give the suppression a reason. Other suppressions in the repo do this, e.g. `// $FlowFixMe workaround for: https://github.com/facebook/flow/issues/...` in `jsutils/isInteger.js`.

**Verification status.** Confirmed by reading. That `'"\\""'` and `String.raw`"\""`` are the same string follows from JavaScript escape semantics. `schemaPrinter-test.js:178` uses the same bare-suppression pattern, where `String.raw` is doing real work across a multi-line template. So this is low severity and is flagged only because, in this case, the suppression is trivially avoidable.

## Non-findings (reviewed and accepted)

- **Hoisting fixtures in `GraphQLError-test.js`.** Five per-test copies of the same `new Source(...)`/`parse` preamble become one module-level fixture (`source`, `ast`, `operationNode`, `fieldNode`), and the `dedent` change shifts the expected offsets and locations consistently (`[4]` / `2:3`, `[6]` / `2:5`, `[0]` / `1:1`). This is a genuine simplification that deletes repetition, and the new expectations are correct: all tests pass.
- **`e: any` in `locatedError-test.js`.** Both tests deliberately build duck-typed, non-`GraphQLError` objects, and the "elasticsearch-like" one assigns a string to `path`. A cast is the honest type here, and `any` is scoped to one local.
- **`new GraphQLError('str')`** replacing `new GraphQLError()`. This is a required-argument fix and is correct.
- **File sizes.** All touched files stay well under 1k lines (largest: `GraphQLError.js` at 220).
