# Located errors, printing, and inspection

## Scope and measurements

Reviewed the complete changed tests and their unchanged owning implementations: `locatedError`, `printError`, and `inspect`. Also inspected AST node field types and the canonical `invariant` helper. These files were reviewed in the primary context; no child reviewer was started.

| Changed file | Base lines | Head lines | New type escapes | New invariants |
| --- | ---: | ---: | ---: | ---: |
| `src/error/__tests__/locatedError-test.js` | 46 | 46 | Two `any` fixture annotations | 0 |
| `src/error/__tests__/printError-test.js` | 96 | 102 | None | 3 |
| `src/jsutils/__tests__/inspect-test.js` | 77 | 78 | One `$FlowFixMe` | 0 |

These counts were produced by a read-only Python script over `git show main:<path>` and head files. `git diff --numstat main...review-head` reports +3/−3, +15/−9, and +2/−1 respectively. None approaches the 1,000-line threshold. The production implementations are identical between the two branches; `git diff --exit-code main...review-head -- src/error/locatedError.js src/error/printError.js src/jsutils/inspect.js` returned exit 0 with no diff.

There are no additional actionable findings in this subsystem. The following sections record the strict structural review and explain why the apparent complexity does not justify a blocker.

## Error boundary fixtures

At `locatedError-test.js:29` and `:41`, `any` permits the existing native Error instances to receive properties outside the built-in Error shape. The first represents an error from another prototype context carrying an array `path`; the second carries an Elasticsearch-style string `path`. Those incompatible input shapes already existed before the PR. The fixtures retain their distinct runtime inputs and assertions.

The canonical behavior remains in `src/error/locatedError.js:25–26`: array paths are treated as the GraphQL-error brand. The wrapping branch remains at lines 29–36. In particular, the string path fixture must not be changed into a conforming GraphQL path just to satisfy Flow, because doing so would reverse the scenario under test. The test suite passes both cases.

The `any` annotations disable static checking for these two local variables, which is a real limitation of this migration. They do not widen a production argument to `any`, add a cast to ordinary execution, or suppress static checks across the file. Without an established failure or a substantial simplification, a new adapter or class hierarchy for two tiny foreign-shape fixtures would not meet the skill's demand to delete complexity.

A worked structural alternative considered was an explicit fixture subclass:

```js
class SearchError extends Error {
  path: string;

  constructor() {
    super('I am from elasticsearch');
    this.path = '/something/feed/_search';
  }
}
```

That would make one extension property explicit, but it adds a class, constructor, and prototype layer to replace one fixture annotation and one assignment. It also changes the fixture's prototype. It is not an obviously better implementation for this diff and is not a remediation request. Widening `locatedError` or introducing a general error-normalization framework would be still less appropriate.

## Printing from two sources

At `printError-test.js:62–64` and `:76–78`, each parsed document's first definition is refined to `Kind.OBJECT_TYPE_DEFINITION`, its optional `fields` array is checked, and its first field is retained. The joint invariant at line 80 checks both field elements before their type nodes are passed to `GraphQLError`.

These checks match `ObjectTypeDefinitionNode` in `src/language/ast.js:430–437`, where `fields` is optional, and the discriminated definition union. They use the existing `Kind` and `invariant` utilities instead of casting an arbitrary AST node into a specific shape. Naming the parsed values `docA` and `docB` also makes their role clearer than the previous `sourceA` and `sourceB` document variables.

The actual input type nodes are unchanged: `fieldA.type` and `fieldB.type` are exactly what the old direct `.definitions[0].fields[0].type` expressions selected. Their source names, field types, highlights, and expected line/column positions remain unchanged. This preserves the cross-source printing case rather than unifying both sources into the shared constructor fixture.

The added conditionals live in fixture setup and state the AST invariant needed for type refinement. They do not add source-specific behavior to the printing path, introduce mode flags, or duplicate production logic. The unchanged printer continues to own source highlighting.

A code-judo alternative considered was extracting a helper that accepts a document and returns its first object-field type:

```js
function firstObjectFieldType(doc) {
  const definition = doc.definitions[0];
  invariant(
    definition &&
      definition.kind === Kind.OBJECT_TYPE_DEFINITION &&
      definition.fields,
  );
  const field = definition.fields[0];
  invariant(field);
  return field.type;
}
```

The helper would centralize two similar checks but retain every assumption, add a new test-specific API and call layer, and require a document type/import if its boundary were made explicit. For only two neighboring fixtures in a 102-line file, it does not delete a meaningful category of complexity. Retaining direct setup is justified. No extraction or schema-building framework is requested.

## Inspection expectation

At `inspect-test.js:31–32`, the only new suppression precedes the existing `String.raw` expectation for inspecting a double-quote character. The input and expected runtime string are unchanged. `inspect` accepts `mixed`, so the migrated test continues to cover its undefined, null, primitive, function, array, object, and custom-inspection branches without changing the production boundary.

The suppression is broader than an ideal precise library typing, but it covers one assertion. A direct literal can express the same expected output without tagged-template machinery: its four code points are `[34, 92, 34, 34]`. The existing raw template makes the escape sequence legible, and a local literal replacement would be a minor syntactic tradeoff. Neither choice creates a dramatic structural simplification, so this is not an actionable finding.

The exact Flow diagnostic motivating the suppression was not available from executed checks. The review does not claim a particular upstream library defect, prove the suppression necessary, or assert that removing it would pass Flow. Resolving that uncertainty would require the static checker, which was not run under this execution allowance.

## Verification and approval bar

The shared focused Mocha command recorded in `01_error_contract_and_fixtures.md` ran the three located-error cases, two printing cases, and nine inspection cases, alongside the constructor tests: **27 passing** in total. No selection was repeated. There was no full-suite or Flow execution, no network request, and no modification of dependencies or the checkout.

The maintainability review found no production branching growth, helper duplication, feature logic leaking into shared paths, non-atomic update, sequential orchestration issue, or unjustified file growth. The added AST checks express existing assumptions at their point of use. The explicit external-error fixtures are small and isolated. An extraction would mainly move these assumptions rather than eliminate them.

Approval of the full PR remains contingent on F1 in the constructor-fixture detail report. This subsystem supplies no additional blocker or unanswered review question.
