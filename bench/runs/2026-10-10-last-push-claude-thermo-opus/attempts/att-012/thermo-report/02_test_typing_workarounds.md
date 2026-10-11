# 02 — Test typing workarounds: `$FlowFixMe`, the printError fixture, and `any` fixtures

Covers summary findings 3, 4 and 5. Files: `src/jsutils/__tests__/inspect-test.js`, `src/error/__tests__/printError-test.js`, `src/error/__tests__/locatedError-test.js`. All line numbers are at the head, `7e39a122eea9292eeffa6905ffdf8a60c5161cfd`.

## Measurements

File sizes at head (`wc -l`): `inspect-test.js` 78, `printError-test.js` 102, `locatedError-test.js` 46. Diffstat: `inspect-test.js` +2 −1, `printError-test.js` +15 −9, `locatedError-test.js` +3 −3.

All three files pass at head (see the command in `01_graphqlerror_contract.md`; `inspect` 9 passing, `printError` 2 passing, `locatedError` 3 passing).

The common thread in this file's findings: each is a place where the conversion to `@flow strict` was achieved by loosening or silencing the checker locally, when a more direct spelling of the same test would have type-checked without help.

## Finding 3 — a blanket `$FlowFixMe` where a plain string literal removes the need for it

### Evidence

`src/jsutils/__tests__/inspect-test.js` lines 28–33 at head:

```js
  it('string', () => {
    expect(inspect('')).to.equal('""');
    expect(inspect('abc')).to.equal('"abc"');
    // $FlowFixMe
    expect(inspect('"')).to.equal(String.raw`"\""`);
  });
```

The suppression pattern in `.flowconfig`:

```
suppress_comment=\\(.\\|\n\\)*\\$FlowFixMe\\($\\|[^(]\\|(\\(<VERSION>\\)?)\\)
```

A bare `$FlowFixMe` therefore suppresses any and all errors reported on the following line, not only the one about the tagged template.

Suppressions in the tree at head (`grep -rn FlowFixMe src`): seven in total. The three in production utilities each state a reason and link an upstream issue (`src/jsutils/objectValues.js` line 15, `src/jsutils/isInteger.js` line 14, `src/jsutils/isFinite.js` line 14). The one precedent for an unexplained suppression on `String.raw` is `src/utilities/__tests__/schemaPrinter-test.js` line 178, from an earlier PR in the same series, where the raw template is a multi-line block passed through `dedent` and is harder to respell.

### Verification status

Confirmed by execution: `'"\\""' === String.raw\`"\""\`` (scratch probe), so the plain literal is an exact replacement. Not verified: the precise Flow error being suppressed, since `flow check` was not run; the finding does not depend on it, because the proposal removes the construct that needs suppressing.

### Why this matters

`String.raw` here saves one backslash in a five-character string. The price paid for that is a line of the test that Flow no longer checks at all, in a file whose only change in this PR is to turn Flow on. The suppressed line contains the call to `inspect`, the function under test; if its signature changes, this is the line that would not report it. An unexplained suppression is also the kind of "temporary" marker that never gets revisited, because nothing records what it was for.

### Worked proposal

```js
  it('string', () => {
    expect(inspect('')).to.equal('""');
    expect(inspect('abc')).to.equal('"abc"');
    expect(inspect('"')).to.equal('"\\""');
  });
```

One line changed, one line deleted, no suppression left in the file.

## Finding 4 — the two-source fixture in the printError test is copy-pasted, misnamed and guarded in three places

### Evidence

`src/error/__tests__/printError-test.js` lines 51–84 at head:

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

At the merge-base each of the two lookups was a single line (`const fieldTypeA = sourceA.definitions[0].fields[0].type;`). The PR does fix one naming problem that existed there, since `sourceA` held a parsed document and is now `docA`. It introduces another: `opA` and `opB` hold `ObjectTypeDefinition` nodes, which the adjacent `Kind.OBJECT_TYPE_DEFINITION` check states outright, so the "operation" prefix is simply wrong.

Three guards are involved: one per block (lines 63 and 77), each combining an existence check, a kind check and a `fields` check in one expression, and then a joint one at line 80 that checks the two `fields[0]` results together, away from where they were produced.

### Verification status

The duplication and naming are confirmed by reading. The proposal below was run as a scratch mocha file under the work directory (`scratch/print-helper-probe-test.js`, without the Flow annotations, since files outside the clone are not transpiled) against the clone's `parse`, `Source`, `GraphQLError` and `printError`; it produces exactly the string the head test expects. Its acceptance by `flow check` was not verified.

### Why this matters

The test's subject is `printError` rendering two nodes from two sources. Nearly thirty lines of setup (lines 52–80) with six intermediate bindings now stand between the `it(...)` line and the first statement that concerns that subject. The narrowing logic is written twice, so a future change to how the node is located must be made twice. This is not a severe problem in a 100-line test file, but it is precisely the copy-paste growth that a typing pass tends to leave behind, and it is cheap to remove now.

### Worked proposal

Extract the one thing both blocks do into a local helper, named for what it returns:

```js
function firstFieldType(body: string, name: string) {
  const typeDef = parse(new Source(body, name)).definitions[0];
  invariant(typeDef && typeDef.kind === Kind.OBJECT_TYPE_DEFINITION);
  const field = typeDef.fields && typeDef.fields[0];
  invariant(field);
  return field.type;
}
```

The test body then states its intent directly:

```js
  it('prints an error with nodes from different sources', () => {
    const error = new GraphQLError('Example error with two nodes', [
      firstFieldType(
        dedent`
          type Foo {
            field: String
          }
        `,
        'SourceA',
      ),
      firstFieldType(
        dedent`
          type Foo {
            field: Int
          }
        `,
        'SourceB',
      ),
    ]);

    expect(printError(error)).to.equal(dedent`
      ...unchanged...
    `);
  });
```

This removes `docA`, `opA`, `fieldA`, `docB`, `opB`, `fieldB` and the detached guard, leaves one narrowing site instead of two and a half, and makes the misnaming impossible because there is nothing left to misname.

## Finding 5 — `const e: any` switches off type checking for the argument the locatedError tests are about

### Evidence

`src/error/__tests__/locatedError-test.js` lines 28–45 at head:

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

The function under test, `src/error/locatedError.js` lines 18–36, declares `originalError: Error | GraphQLError` and performs its own duck-typing with casts placed at the individual property reads: `(originalError: any).nodes`, `(originalError: any).source`, `(originalError: any).positions`.

### Verification status

Confirmed by reading: with `e: any`, the first argument of both `locatedError(e, [], [])` calls is unchecked. Confirmed by execution: assigning the ad-hoc properties onto a plain `Error` and passing it to `locatedError` behaves as the tests expect (scratch probe; the array-`path` error is returned as-is, the string-`path` error is wrapped). Not verified: that Flow 0.86 accepts the property-site form below, since `flow check` was not run. The form mirrors the casts already present in `locatedError.js`, which is the reason for confidence.

### Why this matters

This is the smallest of the five findings and is raised because the remedy is nearly free. Annotating the binding as `any` is the widest available escape hatch: it covers every later use of `e`, including the one call each test exists to make. The tests do need to step outside the `Error` shape, but only at the assignments.

### Worked proposal

```js
  it('does not pass through elasticsearch-like errors', () => {
    const e = new Error('I am from elasticsearch');
    (e: any).path = '/something/feed/_search';

    expect(locatedError(e, [], [])).to.not.deep.equal(e);
  });
```

For the "GraphQLError-ish" test, which assigns five foreign properties, a single widened alias keeps it readable while leaving `e` itself typed:

```js
  it('passes GraphQLError-ish through', () => {
    const e = new Error('I have a different prototype chain');
    const ish: any = e;
    ish.locations = [];
    ish.path = [];
    ish.nodes = [];
    ish.source = null;
    ish.positions = [];
    e.name = 'GraphQLError';

    expect(locatedError(e, [], [])).to.deep.equal(e);
  });
```

In both, the call under test is checked against `Error | GraphQLError` again, and the casts mark exactly the lines that deviate from it.

## Observation not raised as a finding

The narrowing line `invariant(x && x.kind === Kind.…)` now appears in three test files (`GraphQLError-test.js`, `printError-test.js`, and, before this PR, `src/utilities/__tests__/getOperationRootType-test.js`), and twenty files under `src/` still carry `@noflow` at head. If the remaining conversions keep reproducing it, a shared test helper may become worthwhile. Three occurrences do not justify one yet, so this is recorded only as something to watch in the follow-up PRs of this series.
