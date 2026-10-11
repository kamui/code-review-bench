# 01. Error module: `src/error/GraphQLError.js` and `src/error/__tests__/`

Covers findings F1, F2, F4 and F5 from `summary.md`.

Files in scope at the head (`7e39a122e`):

| File | Lines at head | Change |
| --- | --- | --- |
| `src/error/GraphQLError.js` | 220 | +1 / -1 |
| `src/error/__tests__/GraphQLError-test.js` | 150 | +27 / -33 |
| `src/error/__tests__/locatedError-test.js` | 46 | +3 / -3 |
| `src/error/__tests__/printError-test.js` | 102 | +15 / -9 |

Line counts come from `wc -l` on the checked-out head. No file-size concern applies.

## Verification performed

Tests at the head, run from the clone with the babel cache disabled so nothing is written into the tree:

```
BABEL_DISABLE_CACHE=1 ./node_modules/.bin/mocha --require @babel/register --require @babel/polyfill \
  src/error/__tests__/GraphQLError-test.js src/error/__tests__/locatedError-test.js \
  src/error/__tests__/printError-test.js src/jsutils/__tests__/inspect-test.js
```

Result: `27 passing (38ms)`, including `creates new stack if original error has no stack`.

A scratch probe (`clone-work/scratch/stack-branch.js`, outside the clone) constructs a `GraphQLError` with each candidate shape of original error and reports which stack branch was taken:

```
BABEL_DISABLE_CACHE=1 node -r @babel/register -r @babel/polyfill ../clone-work/scratch/stack-branch.js "$PWD"
```

Output:

```
head  (new Error):        { originalHasStack: true,  reusedOriginalStack: true,  stackIsString: true, stackFirstLine: 'Error: original' }
base  ({ message }):      { originalHasStack: false, reusedOriginalStack: false, stackIsString: true, stackFirstLine: 'msg' }
typed (delete e.stack):   { originalHasStack: false, reusedOriginalStack: false, stackIsString: true, stackFirstLine: 'msg' }
typed (stack=undefined):  { originalHasStack: false, reusedOriginalStack: false, stackIsString: true, stackFirstLine: 'msg' }
"{\n  field\n}\n" 4 e
```

`git status --short` was empty before and after both commands.

Not performed: `flow check`. The review's execution allowance permits focused mocha runs only. Every claim below about what Flow accepts or rejects is from reading the source and Flow 0.86 semantics and is labelled "Flow: not executed".

## F1. The "no stack" test lost its stackless fixture

### Evidence

The diff for the test:

```diff
   it('creates new stack if original error has no stack', () => {
-    const original = { message: 'original' };
+    const original = new Error('original');
     const e = new GraphQLError('msg', null, null, null, null, original);
     expect(e.name).to.equal('GraphQLError');
     expect(e.stack).to.be.a('string');
     expect(e.message).to.equal('msg');
     expect(e.originalError).to.equal(original);
   });
```

The production code the test is named after, `src/error/GraphQLError.js` lines 194-209:

```js
  // Include (non-enumerable) stack trace.
  if (originalError && originalError.stack) {
    Object.defineProperty(this, 'stack', {
      value: originalError.stack,
      ...
    });
  } else if (Error.captureStackTrace) {
    Error.captureStackTrace(this, GraphQLError);
  } else {
    Object.defineProperty(this, 'stack', { value: Error().stack, ... });
  }
```

The only other mentions of `.stack` in `src` are the three assertions in this test file (lines 37, 52, 61). So the "original error present, no stack" path into the `else if` had exactly one test, and it is this one.

The probe output above shows the consequence directly. With the head's fixture, `reusedOriginalStack` is `true` and the stack's first line is `Error: original`, the original's stack. With the base's fixture it is `false` and the first line is the new error's own. The head test and the test above it (`uses the stack of an original error`, lines 41-55) now traverse the same branch, and the head test asserts strictly less than its neighbour.

Status: verified by execution.

### Why this is a structural problem and not a nit

The test still passes and still has a title that says the no-stack case is covered. That is worse than deleting the test, because the suite now misreports what it proves. The assertion `to.be.a('string')` was already too weak to pin the behaviour, which is why swapping the fixture went unnoticed; the fix should strengthen the assertion as well as restore the fixture.

### Worked proposal

```js
  it('creates new stack if original error has no stack', () => {
    const original = new Error('original');
    Object.defineProperty(original, 'stack', { value: undefined });
    const e = new GraphQLError('msg', null, null, null, null, original);
    expect(e.name).to.equal('GraphQLError');
    expect(e.stack).to.be.a('string');
    expect(e.stack).to.not.equal(original.stack);
    expect(e.message).to.equal('msg');
    expect(e.originalError).to.equal(original);
  });
```

`original` stays a real `Error`, so it satisfies `originalError?: ?Error` without a cast. `Object.defineProperty` is used instead of `original.stack = undefined` because Flow's library type for `Error.stack` is `string` and the assignment would be rejected; `delete original.stack` is the shorter alternative and reaches the same branch at runtime (probe row three). Flow: not executed for either spelling; confirm with `npm run check`.

With the stack removed, `expect(e.stack).to.be.a('string')` becomes discriminating on its own (a regression that reused the original's stack would yield `undefined`), and the added `not.equal` makes the intent explicit.

## F2. Two copies of the constructor signature, one widened

### Evidence

`src/error/GraphQLError.js` at the head:

```js
declare class GraphQLError extends Error {
  constructor(
    message: string,
    nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void | null,   // line 25, changed by this PR
    source?: ?Source,
    positions?: ?$ReadOnlyArray<number>,
    path?: ?$ReadOnlyArray<string | number>,
    originalError?: ?Error,
    extensions?: ?{ [key: string]: mixed },
  ): void;
```

```js
export function GraphQLError( // eslint-disable-line no-redeclare
  message: string,
  nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void,           // line 94, untouched
  source?: ?Source,
  ...
```

Callers are checked against the `declare class`: the tests pass `null` in the `nodes` position at eight call sites in `GraphQLError-test.js`, `locatedError-test.js` and `printError-test.js`, and the PR needed line 25 to change for those to type-check. The body is checked against line 94, where `null` is still excluded.

The body's handling of the parameter, lines 102-108:

```js
  const _nodes = Array.isArray(nodes)
    ? nodes.length !== 0
      ? nodes
      : undefined
    : nodes
    ? [nodes]
    : undefined;
```

`null` is absorbed by the truthiness test on line 106. That is correct at runtime today, but Flow believes `nodes` in this expression is never `null`, so it would accept a rewrite of line 106 to `nodes !== undefined ? [nodes] : undefined`, after which `_nodes[0]` is `null` and line 114's `node && node.loc` survives but the `reduce` callbacks on lines 119-124 and 134-139 dereference `node.loc` on `null`.

Status: observed by reading. Flow: not executed.

### Why it matters

The whole point of the `declare class` plus function pair is that they describe one contract. The PR title claims to fix the constructor's typing and the fix makes the two descriptions diverge on the exact parameter being fixed. Widening the public type is the right call (the runtime has always accepted `null` here and the four sibling parameters already do), so the remedy is to finish the job, not to revert it.

The spelling adds a second, smaller inconsistency. `?T` in Flow is `T | null | void`. Every other parameter in the list uses it. `nodes` now spells the same thing out by hand as `| void | null`, and since the parameter is already optional (`nodes?:`), the `| void` arm contributes nothing.

### Worked proposal

Apply the same line in both places:

```js
    nodes?: ?($ReadOnlyArray<ASTNode> | ASTNode),
```

at line 25 inside `declare class GraphQLError` and at line 94 in `export function GraphQLError`. No body change is required; the existing truthiness test already handles all three empty cases. If the parenthesised maybe-union is disliked, `nodes?: $ReadOnlyArray<ASTNode> | ASTNode | null` is the minimal alternative, again in both places.

A pre-existing difference between the two copies remains and is out of this PR's diff: `originalError` is `?Error` in the declaration and `?Error & { +extensions: mixed }` in the function. It is the same disease and worth a follow-up, but it was not introduced here.

## F4. Narrowing boilerplate in `printError-test.js`

### Evidence

Head, lines 51-84:

```js
  it('prints an error with nodes from different sources', () => {
    const docA = parse(
      new Source(
        dedent`
          type Foo {
            field: String
          }
        `,
        'SourceA',
      ),
    );
    const opA = docA.definitions[0];
    invariant(opA && opA.kind === Kind.OBJECT_TYPE_DEFINITION && opA.fields);
    const fieldA = opA.fields[0];

    const docB = parse(
      new Source(
        dedent`
          type Foo {
            field: Int
          }
        `,
        'SourceB',
      ),
    );
    const opB = docB.definitions[0];
    invariant(opB && opB.kind === Kind.OBJECT_TYPE_DEFINITION && opB.fields);
    const fieldB = opB.fields[0];

    invariant(fieldA && fieldB);
    const error = new GraphQLError('Example error with two nodes', [
      fieldA.type,
      fieldB.type,
    ]);
```

Three separate issues are visible in this block.

The two halves are textually identical apart from `A`/`B`, `String`/`Int`. The base already had that duplication at one line per side; the head has it at three lines per side plus line 80.

`opA` and `opB` are named as operations, and the same line asserts `Kind.OBJECT_TYPE_DEFINITION`. The sibling file `GraphQLError-test.js` line 23 has `operationNode` checked against `Kind.OPERATION_DEFINITION`, which is the likely source of the name. The base called the parsed documents `sourceA`/`sourceB`, which was also wrong, and the PR correctly renamed those to `docA`/`docB`; it then introduced a new wrong name one line later.

The narrowing of `fieldA` is deferred to line 80, after the whole B block, and merged with `fieldB`'s. The check is far from the value it guards.

Prevalence of the idiom elsewhere, from a search of `src/**/__tests__/*.js` for `invariant(`:

```
src/error/__tests__/GraphQLError-test.js:24:invariant(operationNode && operationNode.kind === Kind.OPERATION_DEFINITION);
src/error/__tests__/printError-test.js:63:    invariant(opA && opA.kind === Kind.OBJECT_TYPE_DEFINITION && opA.fields);
src/error/__tests__/printError-test.js:77:    invariant(opB && opB.kind === Kind.OBJECT_TYPE_DEFINITION && opB.fields);
src/utilities/__tests__/getOperationRootType-test.js:45:  invariant(operationNode && operationNode.kind === Kind.OPERATION_DEFINITION);
src/utilities/__tests__/getOperationRootType-test.js:85:    invariant(schemaNode && schemaNode.kind === Kind.SCHEMA_DEFINITION);
```

`grep -rl "@noflow" src | wc -l` reports 20 files still to convert.

Status: observed by reading; tests pass at the head.

### Worked proposal

```js
function parseFirstFieldType(sourceName: string, body: string): TypeNode {
  const doc = parse(new Source(body, sourceName));
  const typeDef = doc.definitions[0];
  invariant(typeDef && typeDef.kind === Kind.OBJECT_TYPE_DEFINITION);
  const field = typeDef.fields && typeDef.fields[0];
  invariant(field);
  return field.type;
}

  it('prints an error with nodes from different sources', () => {
    const fieldTypeA = parseFirstFieldType(
      'SourceA',
      dedent`
        type Foo {
          field: String
        }
      `,
    );
    const fieldTypeB = parseFirstFieldType(
      'SourceB',
      dedent`
        type Foo {
          field: Int
        }
      `,
    );

    const error = new GraphQLError('Example error with two nodes', [
      fieldTypeA,
      fieldTypeB,
    ]);
```

This restores the base's `fieldTypeA`/`fieldTypeB` names, which said exactly what the values were, removes six intermediate bindings from the test body, puts each `invariant` directly after the value it narrows, and leaves one copy of the narrowing. `TypeNode` would be a type import from `../../language/ast`. Flow: not executed.

Given how many test files are still `@noflow`, it is worth deciding now whether "parse a document and return its first definition narrowed to kind K" belongs in a shared test utility. That is a judgement for the maintainers and larger than this PR; the local helper above is the part that should happen here.

## F5. Binding-wide `any` in `locatedError-test.js`

### Evidence

Head, lines 28-45:

```js
  it('passes GraphQLError-ish through', () => {
    const e: any = new Error('I have a different prototype chain');
    e.locations = [];
    e.path = [];
    e.nodes = [];
    e.source = null;
    e.positions = [];
    e.name = 'GraphQLError';

    expect(locatedError(e, [], [])).to.deep.equal(e);
  });

  it('does not pass through elasticsearch-like errors', () => {
    const e: any = new Error('I am from elasticsearch');
    e.path = '/something/feed/_search';

    expect(locatedError(e, [], [])).to.not.deep.equal(e);
  });
```

`locatedError`'s first parameter is `originalError: Error | GraphQLError` (`src/error/locatedError.js` line 19). With `e: any`, the argument on lines 37 and 44 is unchecked. Two of the file's three tests therefore have no type-checked interaction with the function under test, in a file that now advertises `@flow strict`.

Status: observed by reading.

### Worked proposal

```js
  it('passes GraphQLError-ish through', () => {
    const e = new Error('I have a different prototype chain');
    Object.assign((e: any), {
      locations: [],
      path: [],
      nodes: [],
      source: null,
      positions: [],
      name: 'GraphQLError',
    });

    expect(locatedError(e, [], [])).to.deep.equal(e);
  });

  it('does not pass through elasticsearch-like errors', () => {
    const e = new Error('I am from elasticsearch');
    (e: any).path = '/something/feed/_search';

    expect(locatedError(e, [], [])).to.not.deep.equal(e);
  });
```

`e` remains an `Error` at the call sites, so the calls are checked, and the deliberate shape violation is confined to the one expression that performs it. Flow: not executed. Runtime behaviour is unchanged: `Object.assign` performs the same own-property writes as the six statements it replaces.

## Considered and not raised

The module-scope fixture in `GraphQLError-test.js` (lines 17-26) runs `parse` and two `invariant` calls at import time, so a parser regression would fail the file at load instead of failing a named test. That trade is acceptable for a four-line fixture shared by five tests and the dedupe it buys is worth more.

The `serializes to include message and locations` test previously parsed a bare string (`parse('{ field }')`) and now reuses the shared `Source`-based fixture. `parse` accepting a plain string is covered extensively elsewhere, so nothing meaningful was lost.

`invariant` is declared as `(condition: mixed, message: string)` and is called with one argument throughout the new test code. This matches `src/jsutils/dedent.js` line 20 and the already-typed utilities tests, so it is established practice in this repository and not something this PR should be asked to change.
