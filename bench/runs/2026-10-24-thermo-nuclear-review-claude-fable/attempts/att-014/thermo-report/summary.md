# Thermo Nuclear Review: graphql/graphql-js#3457

Range reviewed: `730d5af8e933235fd5aa312a00be465db0b8acf5..efdbbcfacd92adb78e002f2b8a61f2a6a1504c19` (one commit, "OverlappingFieldsCanBeMergedRule: simplify argument comparison"). One file changed: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`.

## What the change does

The commit removes the `sameArguments` / `stringifyValue` pair, which compared two fields' argument lists by length and then looked each argument of the first list up by exact name in the second list and compared `print(sortValueNode(value))` of the two values. It replaces them with `stringifyArguments(fieldNode)`, which wraps a field's arguments into a synthetic `ObjectValueNode` (one `OBJECT_FIELD` per argument, reusing the argument's `name` and `value` nodes), runs `sortValueNode` over it so fields are ordered by `naturalCompare` on name, and prints it. `findConflict` now reports "they have differing arguments" when the two printed strings differ.

## Verification performed

- `tsc --noEmit` on the full project: clean.
- `eslint` on the changed file: clean.
- Mocha: `OverlappingFieldsCanBeMergedRule-test.ts`, `sortValueNode-test.ts`, `naturalCompare-test.ts`: 53 passing.
- Traced `print` for `ObjectValue` / `ObjectField` (`src/language/printer.ts:119-120`), `sortValueNode` / `sortFields` (`src/utilities/sortValueNode.ts`), `naturalCompare` (`src/jsutils/naturalCompare.ts`) and the `ObjectValueNode` / `ObjectFieldNode` AST shapes (`loc` is optional, so the synthetic node is type- and visitor-safe; the visitor only needs `kind`).
- Ran a scratch script (outside the clone) exercising the new rule through `validate()` with: same arguments in swapped order (no conflict, correct), differing values (conflict, correct), no arguments on both sides (no conflict), one side with arguments and one without (conflict, correct), nested input-object values in swapped order (no conflict), and the natural-compare collision case described below (false conflict, confirmed).

Injectivity of the new string encoding was checked: argument names are `Name` tokens (no commas, colons, braces or whitespace), values are printed canonically with string escaping, and the field order is canonicalised by the sort, so two argument sets print identically only when they are the same multiset of (name, value) pairs, except in the case below where the sort key is not a total order over distinct names. Duplicate argument names are rejected separately by `UniqueArgumentNamesRule`, and the behaviour of this rule on such documents is unchanged in practice (a conflict is reported in both old and new code for the orderings I tried).

## Findings

### Finding 1 (Low): argument names that `naturalCompare` treats as equal are no longer matched by exact name, so a differing argument order produces a false "differing arguments" conflict

File: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, lines 637-650 (`stringifyArguments`), used at line 591.

Evidence: `sortFields` in `src/utilities/sortValueNode.ts` orders fields with `naturalCompare(fieldA.name.value, fieldB.name.value)`. `naturalCompare` accumulates digit runs into a JavaScript number (`aNum = aNum * 10 + aChar - DIGIT_0`), so two distinct names whose digit runs exceed 2^53 and round to the same double compare as `0`. `Array.prototype.sort` is stable, so such fields keep their source order. I confirmed with the scratch script that `naturalCompare('a12345678901234567890', 'a12345678901234567891')` returns `0` in both directions, and that validating

```graphql
{ f(a12345678901234567890: 1, a12345678901234567891: 2)
  f(a12345678901234567891: 2, a12345678901234567890: 1) }
```

against a schema declaring both arguments on `f` now yields `Fields "f" conflict because they have differing arguments...`, even though the two fields have identical argument sets. The removed `sameArguments` looked each argument up by exact `name.value` and would have accepted this document, so this is a behavioural regression introduced by this commit for top-level arguments (nested input-object fields were already subject to the same collision since #3455).

Failure: a valid document is rejected by validation (false positive). Reaching it requires argument names that differ only inside a numeric run of 16 or more digits and appear in different orders on the two fields, which is practically unreachable in real schemas. I rate this Low for that reason; it is reported because it is a real, reproducible divergence from the behaviour the commit claims to merely simplify, and the fix is small (fall back to an ordinal string comparison when `naturalCompare` returns `0` in `sortFields`, or keep exact-name matching for arguments).

## Observations that are not findings

- Every pair of same-response-name fields now builds two synthetic object nodes and runs the printer visitor, even when neither field has arguments. The old code returned immediately for two empty lists. Measured cost is about 5 microseconds per pair in this environment, which is negligible for realistic documents given the rule's existing pairwise behaviour, so this is noted only for awareness.
- No security, devex, or feature-gate impact: the change is internal to a validation rule, adds no inputs, environment variables, dependencies or scripts, and touches no exports.
