# Argument comparison

## Scope and evidence

The only changed file is `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`. At the conflict comparison site, `findConflict` now compares `stringifyArguments(node1)` and `stringifyArguments(node2)` (lines 590–597). The helper at lines 637–650 creates an `ObjectValueNode` whose fields reuse each argument's name and value, then calls `sortValueNode` and `print`.

The former implementation accepted the two argument arrays, compared their lengths, searched the second array by name for every first-array argument, and compared each matched value after sorting and printing it. The replacement removes `sameArguments` and `stringifyValue`; it uses the value-sorting abstraction already imported by this module. The old name matching could perform repeated linear searches. The new representation delegates ordering and recursive nested-value normalization to one existing utility rather than adding a second ordering implementation.

`src/utilities/sortValueNode.ts` sorts object fields by field name, recursively normalizes their values, and recursively normalizes lists. The synthetic outer object therefore treats arguments as unordered and also canonicalizes object values nested within arguments. The existing tests in `src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts` cover both reordered arguments (`allows different order of args`) and reordered input-object fields (`allows different order of input object fields in arg values`, around lines 240–275).

## Measurements

The committed diff is 15 insertions and 27 deletions, for a net reduction of 12 lines. The file is 838 lines on base and 826 lines on head. This remains below the skill's 1,000-line threshold, and the change does not add a new branch to the conflict traversal. The implementation is confined to the existing argument-comparison helper boundary.

Commands used:

- `git diff --stat main...review-head`
- `git diff --unified=80 main...review-head -- src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`
- `wc -l src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`
- `git show main:src/validation/rules/OverlappingFieldsCanBeMergedRule.ts | wc -l`
- `sed -n '1,240p' src/utilities/sortValueNode.ts`
- `sed -n '130,275p' src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts`

## Code-judo assessment

The structurally strongest option here is the implementation already in the patch: represent the set of arguments using GraphQL's existing object-value shape, then normalize it with the canonical value sorter. This deletes the dedicated pairwise matching helper and avoids maintaining separate rules for argument order and nested object-field order. Extracting this helper into another module or introducing a generic canonicalization framework would increase indirection without reducing concepts or behavior-specific branches.

The helper currently constructs and sorts a temporary AST and prints it each time a field pair is checked. This is direct and keeps the normalization semantics obvious. Per-node memoization could reduce repeated work in a heavily reused selection, but the patch offers no measurement showing that this path is a bottleneck, and adding a cache would introduce state and invalidation/lifetime questions. No such change is recommended absent profiling evidence.

## Verification status

Source inspection confirms the helper behavior and the existing tests cover the order-insensitivity contract. No tests were run, and no runtime or performance claim is made. The working tree remained unchanged during the review.

## Finding and remediation

No actionable finding. No remediation is needed; retain the existing sorter/printer implementation. If later profiling identifies repeated normalization as significant, measure it first and then consider memoizing the canonical argument key at an appropriate stable boundary.
