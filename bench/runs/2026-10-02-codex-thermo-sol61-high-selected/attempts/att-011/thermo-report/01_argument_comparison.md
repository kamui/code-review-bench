# Argument comparison and its canonicalization boundary

## Scope and source evidence

This report covers the only changed subsystem: argument equality in `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`. The base is `730d5af8e933235fd5aa312a00be465db0b8acf5`; the head is `efdbbcfacd92adb78e002f2b8a61f2a6a1504c19`. No other reviewers were started. The frozen skill has no mandatory child-call workflow or referenced resources to load.

Source inspection used `git diff main...review-head`, `git show main:src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, numbered reads of the changed rule, and focused reads/searches of its AST contracts, printer, sorter, comparator, validation entry point, uniqueness rule, and tests. Repository guidance files were not loaded as instructions.

The committed delta is 15 insertions and 27 deletions in one file. `git show main:src/validation/rules/OverlappingFieldsCanBeMergedRule.ts | wc -l` reports 838 lines; `wc -l src/validation/rules/OverlappingFieldsCanBeMergedRule.ts` reports 826. The file remains below 1,000 lines and becomes smaller.

At head lines 590–597, `findConflict` compares the two canonical argument strings. At lines 637–650, `stringifyArguments` normalizes an optional argument array, maps each argument to a correctly typed `ObjectFieldNode` inside an `ObjectValueNode`, then calls the existing recursive sorter and printer. The base instead compared array lengths, looked up each argument by name, and separately sorted and printed matching values.

The architectural direction is a useful code-judo move. Arguments and object fields are both unordered name/value collections, so the existing object normalization can remove argument-specific matching. The mapping intentionally changes each node kind while retaining its name and value; it uses no unsafe cast. The helper earns its boundary by hiding that representation conversion from the conflict algorithm. Recursive normalization remains in `src/utilities/sortValueNode.ts`, which is already used by this rule and by `src/utilities/findBreakingChanges.ts:538`.

## Canonical ordering contract

### Finding: [P2] Require a total name order before using sorted arguments as an equality key

The changed line 649 in `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts` makes a sorted and printed whole argument collection its equality key. This is correct only if distinct names have a deterministic ordering independent of source order. `src/utilities/sortValueNode.ts:39–46` sorts object fields using `naturalCompare`; `src/jsutils/naturalCompare.ts:16–29` accumulates digit sequences using ordinary JavaScript numeric arithmetic, and line 50 falls back to the difference in string lengths. Distinct, equal-length names can compare equal when a large numeric suffix loses precision.

The concrete collision is `naturalCompare('a9007199254740992', 'a9007199254740993') === 0`. Both strings are legal GraphQL names. The first numeric suffix is 2^53, and the second rounds to the same JavaScript number. The names have equal lengths and no distinguishing suffix afterward. Stable array sorting keeps the two entries in their incoming order.

Use this entirely valid schema and document:

```graphql
type Query {
  f(a9007199254740992: Int, a9007199254740993: Int): Int
}

{
  f(a9007199254740992: 1, a9007199254740993: 2)
  f(a9007199254740993: 2, a9007199254740992: 1)
}
```

The head creates the respective keys `{a9007199254740992: 1, a9007199254740993: 2}` and `{a9007199254740993: 2, a9007199254740992: 1}`. Their values and argument names are identical, but the strings differ. The head therefore reports `Fields "f" conflict because they have differing arguments. Use different aliases on the fields to fetch both if this was intentional.` The base matches argument names directly and accepts the document.

This is a new use of an insufficient utility contract. The comparator's precision limitation already existed, and nested object normalization already depended on it; the PR newly exposes the limitation to top-level argument ordering. The actionable regression is the new valid-document rejection, anchored at the changed serialization call, rather than a claim that this PR introduced the comparator arithmetic.

Verification is complete for the reproduction. The scratch suite tests the comparator collision directly, compiles the actual pinned base rule in memory, validates the document using that rule alone and with the full specified rule list, and checks that head default validation returns exactly one differing-arguments error. The base and head share the same unchanged parser, schema construction, and validation support code, isolating the change under review.

### Worked code-judo remedy

Keep the new argument-to-object adaptation, and strengthen ordering in the existing value sorter. A deterministic tie breaker avoids adding a second sorting implementation inside the validation rule:

```ts
.sort((fieldA, fieldB) => {
  const nameA = fieldA.name.value;
  const nameB = fieldB.name.value;
  return (
    naturalCompare(nameA, nameB) ||
    (nameA < nameB ? -1 : nameA > nameB ? 1 : 0)
  );
});
```

The lexical fallback establishes a position for different strings even when natural comparison reports a tie. It keeps the natural ordering of names that previously compared distinctly. Plain string comparison avoids locale-dependent ordering. This remedy is small, remains in the canonical layer, and benefits both recursive object normalization and argument normalization.

The scratch suite compiled this exact comparator expression into a private in-memory copy of the sorter and injected it into an in-memory head rule. The large-name reordered document then validated successfully. An ordinary reordered-argument example still validated, and differing argument values still produced a conflict. No production files were changed. This is focused proof of the proposed fix; the entire repository was not tested against this in-memory variant.

Permanent remediation should add the collision to the comparator or sorting utility tests and add a field-merging regression that swaps these distinct argument names. A regression using only ordinary names such as `a` and `b` misses the boundary failure. Run the utility's other consumer tests when changing the shared sorter.

## Empty argument cost

### Finding: [P2] Preserve the constant-cost comparison for empty argument sets

The new comparison at `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts:591` invokes `stringifyArguments` twice per eligible field pair. At lines 639–649 the helper constructs an empty synthetic object, maps an empty array, sorts another empty array, and calls the general AST printer. `src/language/printer.ts:13–14` sends that AST through `visit`. The base's `sameArguments([], [])` returned true after a length check and an empty `.every` call, without printing anything.

This is amplified by the unchanged `collectConflictsWithin` loop at rule lines 477–503, which checks every pair sharing a response name. A valid selection containing 1,000 copies of an argument-free scalar field performs 499,500 such pair comparisons and now makes 999,000 calls to print an empty object. No errors arise, so the validation error limit does not terminate this workload. The same new helper also runs in the between-collection pair loop at lines 527–545.

The focused measurement creates a schema with `f` returning `Int` and optional arguments, then parses `{ ` plus `f ` repeated the chosen number of times plus `}`. It times only `validate(schema, document, [rule])`, excluding schema construction, parsing, loading, and base-rule compilation. Each rule gets one warm-up validation and five measured validations per document; the table gives the median. Every timed validation asserts an empty error list.

| Repeated fields | Base median, ms | Head median, ms | Head with empty signature, ms |
| --- | ---: | ---: | ---: |
| 100 | 3.039 | 30.679 | 1.609 |
| 500 | 17.240 | 685.145 | 18.794 |
| 1,000 | 63.166 | 2,753.398 | 76.731 |

An earlier control run without the remedy measured base/head medians of 2.279/27.766 ms at 100 fields, 16.660/673.941 ms at 500 fields, and 59.483/2,629.394 ms at 1,000 fields. The second run confirms the same substantial change. Absolute timings vary with the host; the source-level extra traversal and the large repeated slowdown are independently observable.

This is a concrete removal of an inexpensive common case, rather than a general objection to serialization or to the preexisting pairwise merge algorithm. It is also a structural quality concern: the simplification funnels an empty collection through a generic AST pipeline whose output is already known. The remedy can keep the new representation while deleting that work.

### Worked code-judo remedy

Return the known signature at the helper boundary before constructing an object:

```ts
function stringifyArguments(fieldNode: FieldNode): string {
  const args = fieldNode.arguments ?? [];
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

Retain the existing FIXME and coverage annotation in a real implementation; they were elided here to focus on the proposed change. The early return yields exactly the printer's empty-object representation, preserving comparisons between omitted and empty argument arrays as well as empty versus nonempty arrays. It belongs in the normalization helper, rather than scattering argument-presence conditions across recursive conflict callers.

The scratch suite compiles an in-memory copy of the actual head source with only this early return added. It uses the same schema, documents, timing loop, and assertions as the base and unmodified head. The last column demonstrates that this small boundary simplification removes the measured regression. A permanent regression check should cover merging two argument-free fields and the existing differing-argument cases; a maintained timing probe can exercise many repeated fields without a flaky absolute-time assertion.

Another structural option is to attach a lazily computed canonical key to each collected field record, so repeated nonempty fields normalize once rather than once per pair. It would require changing the `NodeAndDef` representation or carrying a validation-scoped cache and documenting AST stability. That alternative was not implemented, timed, or elevated into a separate finding: it adds state and touches more of the recursive algorithm than the demonstrated empty-signature fix requires. A module-global cache would introduce lifetime and mutation concerns without a justification in this small refactor.

## Other reviewed invariants

Ordinary argument order and nested input-object order remain insensitive to source ordering, as exercised by the existing field-merging tests and the recursive sorter tests. Lists retain their element order because `sortValueNode` maps list elements rather than sorting the list; scalar and variable nodes are returned unchanged. The printer preserves names and value syntax. No schema coercion or runtime variable comparison was introduced, so the normalization remains an AST-level comparison.

Absent arguments already have the `?? []` behavior at the base, consistent with `FieldNode.arguments?: ReadonlyArray<ArgumentNode>` in `src/language/ast.ts:360`. Moving that fallback into the helper is appropriate and creates no new type ambiguity. The newly constructed object AST meets the declared `ObjectValueNode` and `ObjectFieldNode` contracts without a cast or unchecked generic data shape.

Duplicate argument names can change comparison results because the base searched for the first matching name while the head serializes all entries. Such documents violate the separate uniqueness rule in `src/validation/rules/UniqueArgumentNamesRule.ts`; they are not evidence of an additional valid-document regression. Both actionable reproductions above use valid documents and unique argument names.

The rule's conflict recursion, parent-type exclusivity checks, return-type comparison, caches, and error construction are unchanged. No new ad-hoc conditions are scattered across these paths. There is no asynchronous orchestration or partial-update change to assess. Decomposing the entire 826-line rule is not justified as a blocker for a localized change that reduces its size.

## Commands and verification status

The permitted focused tests used the installed dependencies and `.mocharc.yml`, whose loader is `resources/ts-register.js`. No dependency fetching or upstream browsing occurred.

```sh
./node_modules/.bin/mocha --reporter dot \
  src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts \
  src/utilities/__tests__/sortValueNode-test.ts \
  src/validation/__tests__/UniqueArgumentNamesRule-test.ts

timeout 300 ./node_modules/.bin/mocha --reporter dot \
  src/jsutils/__tests__/naturalCompare-test.ts \
  src/utilities/__tests__/findBreakingChanges-test.ts

timeout 300 ./node_modules/.bin/mocha --reporter spec \
  /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-011/clone-work/review-probes.ts
```

The first group passed 61 tests; the second passed 38. The scratch suite passed five characterization/proposal probes in about 22 seconds. Its defect test deliberately expects the observed erroneous head rejection; this status is not a claim that a permanent regression test expecting acceptance would pass on the head. The final probe log is preserved at `../review-probes.log`, with the reproducible scratch source at `../review-probes.ts`.

The scratch loader reads the pinned base with `git show main:...`, transforms the source using the already installed Babel TypeScript tooling, and compiles it into a private `Module` with the original source directory for resolving imports. Proposed sorter and helper changes are compiled only into in-memory modules. The checkout stays untouched; all scratch files and reports are outside it.

`git diff --check main...review-head` completed successfully. `git status --porcelain=v1` was empty before and after the review, `git diff --exit-code HEAD` succeeded, and the checked-out revision was verified as the pinned head. Loader warnings about installed Browserslist data and the current Node module interpretation did not fail tests and were left unchanged. The full repository build, lint, integration tests, and complete test suite remain unrun under this focused execution allowance.

## Final assessment

The PR succeeds at deleting local matching complexity, keeping normalization in its canonical utility, and maintaining explicit AST types. Approval is blocked by the newly required ordering invariant and the verified cost of serializing empty signatures. Fixing those boundaries preserves the architectural improvement with small, reviewable changes. There are no unresolved questions and no additional actionable findings.
