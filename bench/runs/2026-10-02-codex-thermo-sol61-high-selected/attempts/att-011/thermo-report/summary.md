# graphql/graphql-js#3457: thermo-nuclear review

Review target: `730d5af8e933235fd5aa312a00be465db0b8acf5..efdbbcfacd92adb78e002f2b8a61f2a6a1504c19`, inspected through `git diff main...review-head`. The frozen review skill was applied in one primary context, without delegation, network access, or checkout edits.

## Verdict

Request changes for two actionable findings. The core restructuring is sound: a typed argument-to-object conversion lets the existing recursive sorter replace separate argument matching and value comparison. It deletes complexity and shrinks the rule from 838 to 826 lines. However, the new use requires a stronger sorting contract than the utility provides, and removes the cheap empty-argument comparison from a quadratic field-pair loop. Both regressions were verified against the actual pinned base and head rule implementations.

## Findings

### [P2] Require a total name order before using sorted arguments as an equality key

In `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts:649`, `print(sortValueNode(inputObjectWithArgs))` assumes that sorting gives every distinct argument name a deterministic position. The shared `naturalCompare` accumulates digit runs as JavaScript numbers and returns zero for the distinct valid names `a9007199254740992` and `a9007199254740993`. Stable sorting therefore retains their original order, and swapping these otherwise identical arguments produces different strings. A schema declaring both arguments and the query `{ f(a9007199254740992: 1, a9007199254740993: 2) f(a9007199254740993: 2, a9007199254740992: 1) }` validate successfully at the pinned base but produce a differing-arguments conflict at the head, including with all default validation rules. Give the shared value sorter a deterministic lexical tie breaker for distinct names, or otherwise establish a total ordering before treating its output as canonical; add a regression for reordered argument names with large numeric suffixes. An in-memory tie-breaker proposal restores acceptance without adding an argument-specific sorting path. Full evidence and the worked proposal are in [the subsystem report](01_argument_comparison.md#canonical-ordering-contract).

### [P2] Preserve the constant-cost comparison for empty argument sets

In `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts:591`, comparing two `stringifyArguments` results now allocates, sorts, and prints two empty object ASTs even when both fields have no arguments. `collectConflictsWithin` compares every pair of repeated response names, so this adds printer traversals to all `n(n-1)/2` comparisons of a valid repeated field. In a focused probe using the actual rules, five-sample median validation time for 1,000 repeated argument-free fields rose from 63.166 ms at the base to 2,753.398 ms at the head; 500 fields rose from 17.240 ms to 685.145 ms. Return the constant empty signature from `stringifyArguments` immediately after normalizing its optional arguments, before constructing the synthetic object. This retains the simpler comparison model and avoids the new allocations and traversals; an in-memory implementation reduced the 1,000-field case to 76.731 ms. Add focused verification for the empty-argument path and keep a representative repeated-field timing probe. Full methodology and the worked proposal are in [the subsystem report](01_argument_comparison.md#empty-argument-cost).

## Remediation sequence

First establish deterministic canonical ordering in the shared sorter, with a regression that reproduces the valid-document rejection. Keep argument comparison in its current rule and keep recursive value normalization in the canonical utility. Reintroducing bespoke argument matching would discard the useful simplification.

Then make the empty argument signature a constant before AST construction. This addresses the measured regression locally without threading a new cache through the recursive conflict algorithm. Recheck the field-merging tests, value-sorting tests, comparator tests, and schema-change tests because the sorter has another consumer.

For repeated nonempty arguments, computing a signature once per collected field could avoid repeated normalization. That is an unmeasured design alternative rather than a third finding: it requires a deliberate cache lifetime and AST immutability contract. The verified empty signature remedy should come first.

## Verification and limits

The field-merging, value-sorting, and unique-argument suites passed 61 tests. The natural comparator and schema-breaking-change suites passed another 38 tests. Five scratch probes passed; these are characterization tests that explicitly assert the observed head defect, verify acceptance with the pinned base, exercise an in-memory ordering remedy, and measure base/head/empty-signature timings. Their passing status does not mean the head has fixed either finding.

Measurements used one warm-up and the median of five validations of an already parsed document per rule and field count. All timed validations returned no errors. These synthetic cases establish a repeatable regression in the affected path, not an estimate of overall application latency. Two runs showed the same substantial slowdown. The complete repository test, build, lint, and integration workflows were not run.

No file crosses the 1,000-line boundary. The diff adds no casts, exported API, asynchronous orchestration, or scattered feature checks. The optional argument fallback predates this change and matches the `FieldNode` type. The synthetic object representation is a useful adapter, not an actionable abstraction defect. Differences involving duplicate argument names concern documents already rejected by `UniqueArgumentNamesRule` and are not presented as an additional finding.

The checkout remained clean, `git diff --check main...review-head` passed, and HEAD remained the pinned head. Reproducible probes and captured output are in `../review-probes.ts` and `../review-probes.log`. The [subsystem detail](01_argument_comparison.md) carries commands, source anchors, invariant analysis, and uncommitted remedy proposals. There are no open questions.
