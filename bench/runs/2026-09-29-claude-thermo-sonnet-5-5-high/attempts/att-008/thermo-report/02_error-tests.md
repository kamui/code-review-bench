# Detail 02: error and inspect test migration to `@flow strict`

## Measurements

The commands used were `git diff main...review-head` over the four test files, `grep -n "invariant\|const source\|\[6\]"` on GraphQLError-test, `grep -n "docA\|opA\|invariant"` on printError-test, and `grep -rn FlowFixMe src`. Results: GraphQLError-test has module-level `invariant` calls at lines 24 and 26 and the `[6]` offset at lines 91 and 94; printError-test has narrowing `invariant`s at lines 63, 77 and 80; the only other `$FlowFixMe` in tests is `schemaPrinter-test.js` line 178. All test files remain well under 1000 lines.

## printError-test (finding F2)

Lines 52 to 85 repeat the sequence parse, `definitions[0]`, `invariant(kind === OBJECT_TYPE_DEFINITION && fields)`, `fields[0]` for A and for B. Worked proposal:

```js
function getFirstFieldType(source) {
  const def = parse(source).definitions[0];
  invariant(def && def.kind === Kind.OBJECT_TYPE_DEFINITION && def.fields);
  const field = def.fields[0];
  invariant(field);
  return field.type;
}
```

The test then builds `typeA` and `typeB` with two calls and passes them to `new GraphQLError(...)`. The `opA`/`opB` names (which suggest operations) and the trailing `invariant(fieldA && fieldB)` are gone. Status: not executed (read-only clone); mechanical extraction.

## locatedError-test and inspect-test (finding F3)

`const e: any` at locatedError-test lines 29 and 41 opts the exact values under test out of strict checking. A typed intentional-violation helper (`Object.assign(new Error(...), {...})`) keeps the test's meaning explicit. The bare `// $FlowFixMe` at inspect-test line 31 has no reason attached; the neighbouring precedent at `schemaPrinter-test.js` line 178 has the same shape, so the codebase tolerates it, but a one-line reason or a plain literal expectation is cheaper than a hidden suppression. Status: observed by reading; Flow currently passes with both in place.

## GraphQLError-test (finding F4)

Hoisting the `Source` and AST into module scope deletes three duplicated `parse` blocks and is a net win. Remaining concerns are the import-time `invariant`s, which turn a fixture problem into a file-load failure, and the hand-recomputed offsets (`[4]`, `[6]`, column 3 and 5) that are coupled to the `dedent` fixture without being derived from it. Worked proposal: compute `const fieldStart = source.body.indexOf('field')` next to the fixture and assert against it, and move the narrowing into a small helper. Status: tests pass as written (27 passing); the proposal is unexecuted.
