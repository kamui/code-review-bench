# Thermo-nuclear code quality review: graphql-js #3457

Range reviewed: `730d5af8..efdbbcfa` (`main...review-head`), one commit, "OverlappingFieldsCanBeMergedRule: simplify argument comparison". One file changed, `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, 15 insertions and 27 deletions. The file goes from 838 to 826 lines, so there is no file-size concern.

Review model: `claude-opus-5-5` at `high`, one primary review context, no delegated reviewers and no cross-model review.

## Verdict

Do not approve as written. Request changes.

The commit replaces a pairwise structural comparison of two argument lists (`sameArguments`) with a comparison of two canonical strings (`stringifyArguments`). As an idea this is the right direction: "two fields have the same arguments" becomes "two fields have the same key", and a loop, a nested `find`, and a helper all disappear. The source is shorter and easier to read.

The problem is that the commit stops halfway through its own idea. A canonical string is a property of one field node, but the new code derives it inside the pairwise comparison, so both strings are rebuilt and re-printed for every pair of fields that share a response name. The old code was also pairwise, but it exited before doing any printing when the argument counts differed or when there were no arguments. Those exits are gone, and the argument-less field, which is by far the most common kind, now pays for two full printer traversals per comparison. The measured cost at the rule level is a 24x to 47x slowdown on selection sets that repeat an argument-less field, in a validation rule that runs on untrusted query text and already has a history of performance fixes.

The simplification is worth keeping. It needs one more step to be finished, and that step also makes the rule much faster than it was before the commit.

## Findings

### 1. The canonical argument string is rebuilt for every field pair instead of once per field

In `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, `findConflict` now calls `stringifyArguments(node1) !== stringifyArguments(node2)` at line 591, and `stringifyArguments` (lines 637 to 650) allocates a synthetic `ObjectValueNode`, copies and sorts it through `sortValueNode`, and runs `print`, which is a full `visit()` traversal. `findConflict` is called from the two nested loops in `collectConflictsWithin` (line 490) and `collectConflictsBetween` (line 532), so n fields sharing a response name cost n squared printer runs, and the string computed for a node in one comparison is thrown away and recomputed in the next. The base revision was pairwise too, but `sameArguments` returned on `arguments1.length !== arguments2.length` and did no printing at all for empty argument lists; this commit deletes both exits. Measured with only this rule enabled, 300 repeated argument-less fields validate in 7.4 ms at base and 255.8 ms at head, and 1000 such fields in 60.5 ms at base and 2835.9 ms at head, a 47x regression; 100 repeated `f(a: 1)` fields go from 37.0 ms to 111.1 ms. A 2 KB query of the form `{ s s s ... }` therefore costs about three seconds of validation time, which matters because this rule runs on client-supplied documents and a recent commit on this branch (`4175b26e`, "Prevent Infinite Loop in OverlappingFieldsCanBeMergedRule") was itself a fix for unbounded work in the same rule. The remedy is to finish the reframing rather than revert it: the string is a function of the field node alone, so compute it once per node and memoize it, for example with a `WeakMap<FieldNode, string>` next to `stringifyArguments`, or in a per-rule cache alongside `cachedFieldsAndFragmentNames` if module-level state is unwanted. A prototype of the `WeakMap` version passes all 46 tests in the rule's test file and validates the same workloads in 8.5 ms, 57.9 ms and 2.6 ms respectively, which is on par with base for argument-less fields and 14x to 50x faster than base for fields with arguments. Full measurements, the commands that produced them, and the worked proposal are in `01_overlapping-fields-argument-comparison.md`.

### 2. A commit titled "simplify" changes what the rule reports for duplicate argument names, with no test

The same line, `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts:591`, is not behavior-preserving for fields that repeat an argument name. For `{ f(a: 1, a: 2) f(a: 1, a: 2) }` the base revision reports `Fields "f" conflict because they have differing arguments`, because the old `find` always matched the first `a` and compared `2` against `1`; the head revision reports nothing, because both fields print to the same string. I confirmed this by running both revisions of the rule on that document. The new answer is the better one, since the two fields are textually identical and the duplicate is already reported by `UniqueArgumentNamesRule`, but the commit changes no tests and its title says "simplify", so the next reader has no way to know the change was intended and nothing stops a later rewrite from flipping it back. Add a test to `src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts` that pins the new behavior for identical fields with duplicated argument names, and say in the commit message that the comparison is no longer strictly equivalent to the old one. Evidence is in the "Behavior differential" section of `01_overlapping-fields-argument-comparison.md`.

## Proposed remediation sequence

First, keep `stringifyArguments` and memoize its result per `FieldNode`, as in the worked proposal in the detail file. This is about eight added lines and restores the cost of the argument check to one printer run per field rather than two per pair.

Second, add the duplicate-argument test described in finding 2, so the one intentional behavior difference is recorded.

Third, add a benchmark case to `benchmark/` that validates a selection set with a few hundred repeated fields. The existing `validateGQL-benchmark.js` uses the introspection query, which has almost no repeated response names, and it shows no difference between base and head (0.75 ms against 0.65 ms). That is why this regression is invisible to the repository's current benchmarks.

## Considered and not raised

Wrapping the argument list in a synthetic `ObjectValueNode` so that `sortValueNode` and `print` can be reused is a small disguise: the node has no `loc`, and a reader has to notice that an argument list is being passed off as an input object. I am not raising it as a finding. It reuses the canonical sorter and printer rather than adding a bespoke one, the variable name `inputObjectWithArgs` is honest about what is happening, and once the result is memoized its cost is irrelevant. Moving the `?? []` FIXME into the helper and passing the `FieldNode` rather than a pre-defaulted array is a clear improvement at the call site.

## Verification status

The focused tests pass at head: 48 passing for `OverlappingFieldsCanBeMergedRule-test.ts` and `sortValueNode-test.ts`. The performance numbers and the behavior differential were produced by a scratch script that loads the head rule from the clone and the base rule from `git show main:...`, both against the same checked-out library sources. The proposed remedy was prototyped in the scratch directory and run against a copy of the rule's test file, 46 passing. Nothing in the clone was modified. Type-checking and linting of the proposal were not run.

## Open questions

None.
