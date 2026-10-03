# Thermo-nuclear code quality review: graphql-js #3457

Range reviewed: `730d5af8..efdbbcfa` ("OverlappingFieldsCanBeMergedRule: simplify argument comparison"), one file, `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, +15 / -27.

Reviewed by a single primary context on `claude-opus-5-5` at `high`. No child reviewers were started and no cross-model or alternate-model review was run.

## Verdict

Do not approve as written. There is one actionable finding, and it is a blocker.

The diff is a real reduction in concepts: `sameArguments` plus `stringifyValue` collapse into one `stringifyArguments`, and the file shrinks from 838 to 826 lines. The idea of comparing fields by a canonical argument string is the right one. The problem is where the canonical string is computed. The change puts a full AST sort and print inside the innermost pairwise loop of the rule, and runs it for fields that have no arguments at all. That makes the common case 30 to 47 times slower on the measured inputs, in the validation rule that is already the quadratic one.

The simplification is half of a code-judo move. Finishing it (compute the string once per field, not once per pair) keeps the smaller code, removes the regression, and makes the argument-heavy case about 130 times faster than the base.

## Finding

### 1. Canonical argument string is rebuilt for both fields on every pair comparison, including fields with no arguments

`src/validation/rules/OverlappingFieldsCanBeMergedRule.ts:591` and `:637-650`.

`findConflict` is called once for every pair of fields that share a response name, from the nested loops in `collectConflictsWithin` and `collectConflictsBetween`. At the base, the argument check in that loop was cheap when there was nothing to compare: `sameArguments` compared two lengths and ran `every` over an empty array. At the head, line 591 calls `stringifyArguments` on both nodes for every pair. Each call allocates a synthetic `ObjectValueNode`, maps the arguments into new `ObjectFieldNode` objects, copies and sorts them in `sortValueNode`, and then runs `print`, which is a full `visit` traversal with the printer reducer. For two fields with no arguments this work produces the string `{}` twice and compares them.

The cost is measured, not inferred. Validating a single selection set that repeats an argument-less field, with only this rule enabled, takes 60 ms at the base and 2832 ms at the head for 1000 repetitions, 17 ms against 710 ms for 500, and 5 ms against 173 ms for 250. That is a 30x to 47x slowdown on the path every query takes, since most fields have no arguments. For fields that do carry arguments the head is about 1.2x to 1.4x slower than the base (2167 ms against 2671 ms for 200 repetitions), because the base already paid for a print per argument value per pair.

This matters beyond throughput. The rule's cost is controlled by the document author, so a larger constant on the quadratic term directly lowers the size of query needed to hold a server's validation step for seconds. The preceding commit on this branch (`4175b26e`, "Prevent Infinite Loop in OverlappingFieldsCanBeMergedRule") was hardening this same rule.

The remedy is to finish the reframing the diff started. The canonical string is a property of one field node and does not depend on the field it is compared with, so it should be computed at most once per node and skipped entirely when the node has no arguments. Keep `stringifyArguments` as the single concept, return early with an empty string when `args.length === 0`, and memoize the result per `FieldNode`. I prototyped exactly that outside the clone. With the early return alone, the argument-less case returns to base speed (74 ms for 1000 repetitions) but the argument-heavy case stays at 2553 ms. With the early return plus a per-node memo, the argument-less case is at base speed (66 ms) and the argument-heavy case drops from 2167 ms at the base to 16 ms for 200 repetitions. All 521 tests under `src/validation/__tests__` pass against both prototypes.

The memo can be a module-level `WeakMap<FieldNode, string>` as prototyped, or a `Map` created in the rule's `SelectionSet` visitor next to `cachedFieldsAndFragmentNames` and threaded the same way. The second is more consistent with how this file already owns its per-validation caches, at the cost of one more parameter on an already long parameter list. Either is acceptable; recomputing per pair is not.

The change also ships with no benchmark or test that would have caught this. `benchmark/` has `validateGQL-benchmark.js` and `validateInvalidGQL-benchmark.js`, but neither exercises many fields sharing a response name. A small benchmark of that shape would keep this from regressing again.

Full measurements, commands, and the worked proposal are in [01_overlapping_fields_argument_comparison.md](01_overlapping_fields_argument_comparison.md).

## Question for the author

Is the changed result for duplicated argument names intended, and should a test pin it? For `{ f(a: 1, a: 2) f(a: 1, a: 2) }`, the base reports `Fields "f" conflict because they have differing arguments` and the head reports nothing, because the base looked up each argument by name with `find` and always hit the first `a`. The head's answer is the better one (the two fields are textually identical, and `UniqueArgumentNamesRule` reports the duplicate on its own), but the commit is titled as a simplification and no test covers the case in either direction. Evidence is in the detail file under "Behavior comparison".

## Checked and not raised

The file-size rule does not apply: the file goes from 838 to 826 lines and stays under 1000.

The diff adds no new branches, flags, optional parameters, or casts. The `/* c8 ignore next */` fallback for `arguments` and its FIXME moved with the code and did not multiply.

The synthetic `ObjectValueNode` at lines 641-648 dresses an argument list up as an input object so that `sortValueNode` and `print` can be reused. It is a little indirect, and a reader has to know that an `OBJECT` prints as `{a: 1}`, but it reuses the two canonical helpers instead of adding a bespoke sorter, and the resulting strings distinguish every case I tried. I do not consider it actionable on its own. If the memoized version in finding 1 is adopted, this construction ends up in one place that runs once per field, which is where it belongs.

Argument order independence is preserved and is covered by the existing test "allows different order of args".

## Remediation sequence

1. Add the empty-arguments early return to `stringifyArguments`. This alone removes the 30x to 47x regression for argument-less fields.
2. Memoize the canonical string per `FieldNode` so each field is sorted and printed once per validation. This turns the argument comparison from a quadratic number of prints into a linear number.
3. Add a benchmark under `benchmark/` that validates a selection set with many fields sharing one response name, with and without arguments.
4. Decide the duplicated-argument-name behavior raised in the question above and pin it with a test.

## Verification status

The focused tests pass at the head: 48 passing for `OverlappingFieldsCanBeMergedRule-test.ts` plus `sortValueNode-test.ts`. The timings and the behavior comparison were produced by scratch scripts run against copies of the base and head trees in the work directory; the clone was not modified and `git status` is clean at `efdbbcfa`. Timings are from Node v24.21.0 on one machine, averaged over three validations after three warm-up runs, and should be read as ratios.
