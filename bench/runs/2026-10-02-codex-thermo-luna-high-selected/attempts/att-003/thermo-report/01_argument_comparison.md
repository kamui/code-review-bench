# Argument comparison in overlapping fields

## Scope and measurements

The reviewed range is `730d5af8e933235fd5aa312a00be465db0b8acf5..efdbbcfacd92adb78e002f2b8a61f2a6a1504c19`. The sole changed file is `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts` (15 insertions and 27 deletions). At head it contains 826 lines, so the patch does not push it over the skill's 1,000-line decomposition threshold.

Previously, `sameArguments` checked argument counts and searched the second argument array by name for each first-side argument, then compared each value after sorting and printing. The new `stringifyArguments` maps the field's argument nodes into an `ObjectValueNode`, then reuses `sortValueNode` and `print` to produce a canonical string. `sortValueNode` sorts object fields by name recursively and leaves list item order intact. This represents the unordered nature of argument lists and input-object fields using the existing value canonicalizer.

## Finding status

No actionable maintainability finding. The new helper has a clear, narrow purpose and eliminates bespoke matching logic instead of adding a branch or spreading policy across the validator. The synthetic object is a direct fit for `sortValueNode`'s existing input shape; adding a generalized argument-sorting abstraction would create a new contract without removing meaningful complexity.

## Code-judo assessment

I considered whether argument comparison should instead gain a dedicated canonicalization utility or compare structured argument maps directly. A new utility would duplicate or broaden `sortValueNode`; a direct map comparator would restore custom comparison logic. The implementation in the patch is the simpler ownership boundary: the rule converts the argument list to the existing object-value representation, and the established utility handles recursive ordering. No clearer restructuring is evident from this localized diff.

## Verification status

Executed:

- `./node_modules/.bin/mocha --full-trace src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts` — passed, 46 tests, including “allows different order of args” and “allows different order of input object fields in arg values.”
- `git diff --check main...review-head` — passed with no output.
- `wc -l src/validation/rules/OverlappingFieldsCanBeMergedRule.ts` — 826 lines at head.

The test runner emitted existing environment notices about outdated `caniuse-lite` data and module-type inference; they did not affect the test result.
