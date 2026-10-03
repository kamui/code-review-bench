# Argument comparison

## Scope and measurements

The change is limited to `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`. The comparison in `findConflict` now compares `stringifyArguments(node1)` and `stringifyArguments(node2)` at lines 590–597. `stringifyArguments` builds an `ObjectValueNode` from argument names and values, passes it through the shared `sortValueNode`, then prints it at lines 637–650.

The prior implementation had `sameArguments`, which checked lengths and searched the other argument list by name, plus `stringifyValue` for individual values. The replacement removes these two helpers and uses the shared recursive canonicalizer for both argument names and nested object input values. The diff is 15 insertions and 27 deletions (12 net lines removed); the file is 826 lines after the change, so it does not approach the 1,000-line threshold.

## Structural review and code-judo assessment

This is a sound simplification. GraphQL arguments and input-object fields are unordered named collections, while list values remain ordered. Modeling arguments as an object-shaped value lets the existing `sortValueNode` encode precisely those rules recursively: it sorts object fields but leaves list order intact. The helper no longer carries its own argument matching algorithm or duplicates value canonicalization.

The implementation is direct and local to the comparison that owns the behavior. The temporary object AST is a small adapter to the existing canonical representation, not a new general-purpose abstraction. A more ambitious alternative such as caching normalized strings would add state and invalidation concerns to a validation path without evidence that this focused conversion is a bottleneck; it is not justified by this change.

Existing focused tests cover differing and missing arguments, conflicting values and names, argument order independence, and nested input-object field order independence (`src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts`, around lines 144–275). Those cases align with the new representation. No issue was found in the inspected sorting implementation: `sortValueNode` recursively sorts object fields and preserves list element order.

## Verification status

- Inspected the committed range with `git diff main...review-head -- src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`.
- Inspected `src/utilities/sortValueNode.ts` and the focused rule tests.
- `git diff --check main...review-head` completed without output.
- Tests were not run.
- The checkout reported no working-tree changes.

## Findings and proposed remediation

No actionable finding. The code-judo move is already present in the patch: remove the bespoke comparison layers and express unordered arguments through the shared object-value normalization behavior. No further restructuring is justified by the evidence in this diff.
