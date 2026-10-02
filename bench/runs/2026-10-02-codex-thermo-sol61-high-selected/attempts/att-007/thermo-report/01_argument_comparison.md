# Argument comparison and canonicalization

## Scope and judgment

This detail report covers the only changed subsystem: argument equality in `OverlappingFieldsCanBeMergedRule`. Request changes for two demonstrated regressions. The code is shorter and its abstraction is locally coherent, but the new canonicalization requires a total argument-name order and imposes unnecessary general-purpose AST work on every empty-argument comparison.

The pinned base is `730d5af8e933235fd5aa312a00be465db0b8acf5`; the pinned head is `efdbbcfacd92adb78e002f2b8a61f2a6a1504c19`. Initial and final Git status were clean, and `git rev-parse HEAD main` matched both pins. The change was read using `git diff main...review-head`; `git diff --check main...review-head` reported no whitespace errors.

No reviewers were delegated. Only the requested frozen skill, task packet, execution policy, and repository source relevant to the change were used. No network access was needed. Scratch tests and this report live under `clone-work`, outside the read-only clone.

## Source measurements and architecture

`git diff --stat main...review-head` and `git diff main...review-head --numstat` establish one changed file, with 15 insertions and 27 deletions. `wc -l src/validation/rules/OverlappingFieldsCanBeMergedRule.ts` reports 826 lines; `git show main:src/validation/rules/OverlappingFieldsCanBeMergedRule.ts | wc -l` reports 838. There is no threshold crossing or file-size explosion.

At the base, `findConflict` extracts two argument arrays and calls `sameArguments`. That function first compares argument counts, then looks up each argument by its exact name and compares `print(sortValueNode(argument.value))`. This has potentially quadratic argument-name searches, but an empty argument list requires neither sorting nor printing. A missing name, unequal count, or early unequal value also short-circuits.

At the head, `findConflict` line 591 compares two `stringifyArguments` results. Lines 637–650 normalize missing arguments to an empty array, construct an explicitly typed synthetic object with one object field per argument, recursively sort it, and print the result. Argument names and input-object field names now share a sorting path.

The comparator is called from both the within-selection-set loop at lines 479–501 and the between-collections loop at lines 533–550. The former enumerates all pairs of fields sharing a response name; the latter enumerates the Cartesian product of corresponding collections. Argument serialization happens after the field-name check and only when the types are not mutually exclusive. Those gates are unchanged.

The synthetic node remains internal and has the correct shape: `FieldNode.arguments` is optional and readonly (`src/language/ast.ts:355–363`), while `ObjectValueNode.fields` holds object fields with a name and value (`ast.ts:478–495`). No location is needed for printing, and `sortValueNode` returns copies rather than mutating the input AST. The existing optional-argument fallback is moved, not introduced. No casts or untyped public boundaries are added.

`src/utilities/sortValueNode.ts:13–44` recursively sorts object fields while preserving list-element order. It uses `naturalCompare` at lines 44–46. `src/language/printer.ts:16–18` delegates to the general AST visitor; object printing at lines 118–119 joins printed names and values. Argument-to-object conversion is therefore sufficient for ordinary legal unique argument names, provided the sorting order distinguishes them.

## Finding 1: canonical order is not total

The changed anchor is `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts:649`. The new whole-argument signature inherits an ordering assumption that the previous exact-name lookup did not have. The implicated existing helpers are `src/utilities/sortValueNode.ts:44–46` and `src/jsutils/naturalCompare.ts:16–37,50`.

`naturalCompare` accumulates numeric runs into JavaScript numbers. The distinct strings `a9007199254740992` and `a9007199254740993` have the same length and numeric runs that round to the same number. Neither numeric comparison returns a nonzero result, and the final string-length subtraction also returns zero. Stable sorting preserves the input order for this pair. Equality canonicalization needs a consistent order for distinct names, so preserving the original order breaks the argument-order invariant.

The schema and document are both legal GraphQL:

```graphql
type Query {
  f(a9007199254740992: Int, a9007199254740993: Int): Int
}
```

```graphql
{
  f(a9007199254740992: 1, a9007199254740993: 2)
  f(a9007199254740993: 2, a9007199254740992: 1)
}
```

The first signature is `{a9007199254740992: 1, a9007199254740993: 2}`; the second reverses those fields. Full specified-rule validation at the base produces zero errors. The head produces one overlapping-fields error claiming differing arguments. This is a newly exposed top-level argument failure, even though the same comparator already existed for nested input-object fields. It is not a complaint about a wholly pre-existing bug.

Verification status: reproduced and asserted against pinned base and head code. Both an exact-name tie-break and direct lexical sorting accept the document. The reproduction also asserts that `naturalCompare` returns zero for the two names. Existing tests only permute ordinary names such as `a` and `b`, explaining why they pass.

Remediation: strengthen the ordering in the canonical sorting layer, and add the schema and query above to the overlap-rule tests. Preserve recursive object normalization and list order. Do not add a large-number special case in `findConflict`; that would scatter knowledge of the sorter's implementation into the validation flow.

## Finding 2: empty arguments enter the printer for every pair

The changed caller anchor is `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts:591`; the new unconditional object construction and printing are at lines 639–649. This shifts the normal empty-input case from an empty-array comparison to repeated general AST traversal.

For `type Query { f: Int }`, `{ f f ... }` with any number of repeated `f` selections is valid. With 500 selections the existing rule compares 500 × 499 / 2 = 124,750 pairs. The head serializes two empty synthetic objects per pair, totaling 249,500 serializations, even though every argument signature must be identical. The old rule performs no argument-value serialization in this case. The refactor leaves the pair count unchanged; the regression is the cost added inside that loop.

The scratch timing test uses full specified-rule validation, not an isolated helper benchmark. Schema construction and parsing occur once, outside the timed region. It warms each base/head/remedy variant five times, then takes seven samples per variant in rotating order. Each validation result is asserted to contain zero errors. The report uses medians in milliseconds:

| Repeated fields | Compared pairs | Base | Head | Empty fast path plus name tie-break | Head/base |
| --- | ---: | ---: | ---: | ---: | ---: |
| 100 | 4,950 | 1.951856 | 28.990665 | 2.300028 | 14.85× |
| 500 | 124,750 | 20.439104 | 718.855758 | 23.674766 | 35.17× |

The name comparator is never needed for an empty argument list, so the measured candidate's improvement in these cases comes from its early empty-signature return. The benchmark preserves the ordinary validator traversal and pre-existing quadratic pair comparisons.

Verification status: measured on the supplied installed environment with identical head dependencies for all variants. Only the changed rule source is substituted for the base comparison; this PR changes no other files. Timing is environment dependent and the workload is synthetic. The claim is limited to this valid repeated-field path, not to all GraphQL documents or a production denial-of-service threshold. The timing command finished in about nine seconds, within the five-minute allowance.

Remediation: return `'{}'` immediately for zero arguments. Add a focused repeated-field performance check or benchmark to protect the common empty-input path. The rule's existing loops and cache responsibilities do not need to change. The loss of count and value short-circuits on nonempty mismatches is visible in the source, but was not separately benchmarked and is not an additional actionable finding here.

## Worked remedies

The strongest simplification for name canonicalization is to use direct string ordering in `sortFields`. There is no need for numeric interpretation to decide whether two sets of uniquely named GraphQL arguments are equal. Replace the natural-order comparator in the canonical utility with this comparison and remove its `naturalCompare` import:

```ts
.sort((fieldA, fieldB) => {
  const a = fieldA.name.value;
  const b = fieldB.name.value;
  return a < b ? -1 : a > b ? 1 : 0;
});
```

This eliminates the numeric-run parsing dependency from the equality representation instead of adding special cases for particular argument names. Repository searches found `sortValueNode` consumers in this rule and in `findBreakingChanges`, both using printed sorted values for equality, and its own tests. It is marked `@internal` and is not re-exported by `src/index.ts` or `src/utilities/index.ts`. Nevertheless, inspect canonical-output expectations in the breaking-change and sorter tests before landing a change to ordering.

If the existing natural presentation order must be preserved, the narrower comparator change is:

```ts
.sort((fieldA, fieldB) => {
  const a = fieldA.name.value;
  const b = fieldB.name.value;
  return naturalCompare(a, b) || (a < b ? -1 : a > b ? 1 : 0);
});
```

This preserves existing nonzero natural comparisons while resolving zero results for unequal names. Both variants were exercised in memory against the new reproduction and the eight listed value cases. The lexical proposal changes canonical ordering for numeric names more generally; the tie-break proposal changes only collisions. Neither was committed or applied to disk in the clone.

The second remedy is independent and local:

```ts
function stringifyArguments(fieldNode: FieldNode): string {
  // FIXME https://github.com/graphql/graphql-js/issues/2203
  const args = /* c8 ignore next */ fieldNode.arguments ?? [];
  if (args.length === 0) {
    return '{}';
  }

  const inputObjectWithArgs: ObjectValueNode = {
    kind: Kind.OBJECT,
    fields: args.map((argNode) => ({
      kind: Kind.OBJECT_FIELD,
      name: argNode.name,
      value: argNode.value,
    })),
  };
  return print(sortValueNode(inputObjectWithArgs));
}
```

The return matches the existing printer's empty-object representation. It also handles omitted `arguments`, because the current fallback normalizes that input to an empty array. This one guard deletes mapping, sorting, allocation, and general visitor work from every empty call without introducing shared mutable state, global caches, or additional parameters throughout fragment traversal.

Restoring the base helper wholesale would preserve the demonstrated behavior and avoid this serialization cost, but would also restore the bespoke name-search loop that the PR aims to remove. A cache could amortize nonempty signatures, but introduces ownership and lifetime decisions and is unnecessary for the two verified defects. Prefer the canonical order repair and simple empty-input return first. A broad rule decomposition is not warranted by a 12-line net reduction in a coherently documented algorithm.

## Commands and verification record

All commands ran from the supplied clone directory, except artifact writes to `clone-work`. Relevant source was read with `rg`, `sed`, `nl`, `cat`, and `git show`. No ambient guidance or additional skill resources were loaded; the frozen skill references no mandatory resource for this workflow.

The initial focused repository check was:

```sh
./node_modules/.bin/mocha src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts src/utilities/__tests__/sortValueNode-test.ts src/language/__tests__/printer-test.ts
```

Result: 57 passing tests. To cover the canonical utility's other equality consumer and the implicated name comparator, the focused check was broadened once:

```sh
./node_modules/.bin/mocha --reporter dot src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts src/validation/__tests__/UniqueArgumentNamesRule-test.ts src/utilities/__tests__/sortValueNode-test.ts src/utilities/__tests__/findBreakingChanges-test.ts src/jsutils/__tests__/naturalCompare-test.ts src/language/__tests__/printer-test.ts
```

Result: 108 passing tests in 65 ms of reported test time. The loader emitted existing Browserslist age and Node module-type warnings; neither caused a failure. No attempt was made to update dependencies or configuration.

The self-contained evidence file is [argument-comparison-review-test.js](../argument-comparison-review-test.js). It reads the base rule with `git show main:...`, reads the head rule from disk, and uses the repository's Babel transform to compile both as in-memory modules with the same relative dependencies. Its full-rule variant substitutes only `OverlappingFieldsCanBeMergedRule` in `specifiedRules`. Comparator and empty-signature candidates are likewise injected only into in-memory modules.

```sh
./node_modules/.bin/mocha /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-007/clone-work/argument-comparison-review-test.js
```

Result: three passing tests in nine seconds. The tests assert the false-conflict reproduction, exercise eight value boundaries, and measure the two repeated-field sizes. After adding the lexical comparator candidate, the correctness checks were repeated without re-running unchanged timing cases:

```sh
./node_modules/.bin/mocha /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-007/clone-work/argument-comparison-review-test.js --grep 'reproduces|boundaries'
```

Result: two passing tests in 36 ms. Boundary coverage includes empty arguments, reordered arguments and nested input-object fields, ordered lists, missing versus explicit null arguments, different string values, identical block strings, distinct variables, and the existing distinction between block and ordinary string printing. The last distinction is preserved by the candidates; it is not introduced by this diff.

The full integration suite, project lint/type checks, and candidate versions of all repository tests were not run. Existing focused tests establish baseline compatibility, not the absence of the newly reproduced defects. The worked remedies have focused reproduction coverage and need normal project verification when implemented.

## Remaining questions

None. The two actionable findings are stated in the summary. No cosmetic nits, pre-existing giant-file complaints, alternate-model reviews, or speculative regressions are promoted to findings.
