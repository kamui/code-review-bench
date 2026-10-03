# Review of graphql/graphql-js#3457

Request changes. The refactor is structurally smaller and reuses the canonical value normalizer, but it introduces one verified correctness regression and one substantial performance regression. Both have narrow remedies that preserve the proposed simplification.

The review covers only `730d5af8e933235fd5aa312a00be465db0b8acf5..efdbbcfacd92adb78e002f2b8a61f2a6a1504c19`, inspected with `git diff main...review-head`. One file changes, with 15 insertions and 27 deletions. The frozen thermo-nuclear-code-quality-review workflow was followed in a single primary context; no reviewers were delegated and no network requests were made.

## [P2] Require deterministic ordering before comparing serialized arguments

In `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, lines 643–649, converting the arguments into an object and comparing its sorted printout assumes that `sortValueNode` gives distinct argument names a deterministic order. Its `naturalCompare` comparator returns zero for the distinct, legal names `a9007199254740992` and `a9007199254740993` because their numeric suffixes round to the same JavaScript number. Sorting therefore preserves their original order, and two calls with identical name/value pairs in reverse argument order produce different strings. A complete valid schema and query pass all specified validation rules with the pinned base rule but fail on the head with “they have differing arguments.” The comparator weakness already affected nested input objects; this change newly exposes top-level argument matching to it. Give the canonical sorter a lexical tie breaker for distinct names, or use another exact total ordering, and add regression coverage for reordered arguments with these names. The scratch experiment verifies that a tie breaker fixes the query while preserving nested object normalization and list order. Full evidence and the worked remedy are in [the argument-comparison detail report](01_argument_comparison.md#finding-1-deterministic-name-ordering).

## [P2] Preserve the empty-argument fast path in the pairwise loop

In `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, lines 637–649, `stringifyArguments` now allocates, sorts, and prints an empty synthetic object even for fields with no arguments. Its caller at line 591 invokes it twice for every overlapping field pair. The base compares two empty argument arrays without visiting any value AST, so this introduces expensive serialization into the common argumentless case inside the existing quadratic loop. In warmed focused tests of a valid query containing 600 repeated `f` fields, the base median was 22.85 ms and the head median was 1,039.43 ms across seven samples; every validation returned no errors. Return the canonical empty-object string immediately when the argument array is empty, keeping the guard inside this helper. A scratch version of that guard measured 47.82 ms for the same 600-field fixture and still handled missing AST argument arrays and differing arguments correctly. This is a substantial synchronous validation regression, not a reason to redesign the entire conflict algorithm. Full measurements, reproduction commands, and the worked remedy are in [the argument-comparison detail report](01_argument_comparison.md#finding-2-empty-argument-serialization-in-the-pairwise-loop).

## Structural assessment

The rule shrinks from 838 to 826 lines and does not cross the 1,000-line threshold. Replacing the name-search loop and per-value helper with one typed `ObjectValueNode` genuinely deletes complexity. The optional argument-array fallback is relocated unchanged, and no new casts, flags, orchestration, or architectural layers are added. The synthetic object is a useful way to reuse the existing normalizer; the two findings concern its ordering contract and its cost at the call site. There is no separate decomposition, wrapper, or spaghetti-growth finding.

## Remediation sequence

First make the canonical field-name ordering deterministic, retaining the shared recursive normalizer rather than introducing a second argument-specific equality implementation. Add the large numeric-suffix regression to the existing argument-order tests and cover ties in the sorter.

Then restore the cheap empty-argument path inside `stringifyArguments`. Keep its existing missing-array fallback and ensure empty versus nonempty argument lists still conflict. No broad rewrite or new cache is required for the demonstrated regression.

Rerun the three focused suites, the new correctness regression, and the focused repeated-field timing check. If the shared sorter is changed, also exercise its other consumer, `findBreakingChanges`, before merging. The detailed report contains the exact worked proposals and their verification limits.

## Verification and artifacts

The unchanged head passes all 53 existing tests in the overlapping-fields rule, value sorter, and natural comparator suites. The scratch correctness fixture has four passing tests and one expected failure: the assertion that the head accepts the valid reordered-argument query. The base accepts that query under all specified rules. Separate focused cost and empty-argument-guard checks pass. Proposed remedies were compiled and tested only in memory; nothing was applied to the checkout.

Absolute timing is environment dependent. The measurements compare warmed executions against the same parsed AST and schema, with alternating base/head sample order. They establish the removed fast path's impact on this fixture, not a universal slowdown factor for all GraphQL queries. The full project test, lint, type-check, and integration workflows were not run. The checkout remained clean with unchanged pinned branch identities.

- [Subsystem evidence, measurements, and worked code-judo proposals](01_argument_comparison.md)
- [Exact finding locators](finding-index.json)
- [Scratch reproduction and remedy experiments](../argument-order-regression-test.js)

