# Thermo-nuclear review: graphql/graphql-js#3457

Range: `730d5af8e933235fd5aa312a00be465db0b8acf5..efdbbcfacd92adb78e002f2b8a61f2a6a1504c19` ("OverlappingFieldsCanBeMergedRule: simplify argument comparison"). One file changed: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`.

The change replaces `sameArguments` (length check, then per-argument lookup by name and comparison of the sorted, printed values) with `stringifyArguments`, which wraps a field's arguments in a synthetic `ObjectValueNode`, sorts it with `sortValueNode`, prints it, and compares the two strings.

I read the changed rule, `sortValueNode`, `naturalCompare` and the printer, ran the focused Mocha suites at head (`OverlappingFieldsCanBeMergedRule-test.ts`, `sortValueNode-test.ts`, `naturalCompare-test.ts`: 53 passing), and ran a differential probe and timing script against exported copies of base and head in the work directory. Two findings follow. The clone was not modified.

## Findings

### 1. [Medium] Every field-pair comparison now sorts and prints both argument lists, making validation of repeated argument-less fields about 40x slower

File: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, lines 591 and 637-650.

`findConflict` runs once per pair of fields that share a response name (`collectConflictsWithin` and `collectConflictsBetween`), so it is the quadratic inner step of this rule. At base, two fields without arguments cost almost nothing there: `sameArguments([], [])` compared lengths and returned from `[].every(...)` without printing. At head, line 591 calls `stringifyArguments` on both nodes unconditionally, and each call allocates a synthetic object node, copies and sorts its fields, and runs a full `visit`-based `print` (lines 641-649). That happens for every pair, including the overwhelmingly common case of fields with no arguments, and nothing is cached per field node, so a field appearing in N pairs is stringified N times.

Measured with `validate(schema, doc, specifiedRules)` on the same machine, base vs head:

| Document | Base | Head |
| --- | --- | --- |
| `{ a a a ... }`, 100 fields (203 bytes) | 3.4 ms | 32.4 ms |
| `{ a a a ... }`, 1000 fields (2 KB) | 67 ms | 2729 ms |
| `{ a a a ... }`, 2000 fields (4 KB) | 273 ms | 11171 ms |
| 200 fragments each selecting `id name` on the same parent (8 KB) | 31 ms | 241 ms |
| `{ f(a:1) f(a:1) ... }`, 500 fields (3.5 KB) | 874 ms | 2796 ms |

Failure: validation is synchronous and this rule is in `specifiedRules`, so a 4 KB query of a repeated plain field now blocks the event loop for about 11 seconds on any schema, where it took about 0.3 seconds before. Legitimate documents that repeat `id`/`__typename`-style fields through many fragments also pay roughly 8-40x more validation time. The quadratic pair count is not new, and base already printed values per pair for fields that have arguments (874 ms row above), so this is a worsening of an existing cost rather than a new class of attack; that is why I rate it Medium rather than High. The regression is specifically that the cheap path for argument-less fields is gone and the argument-bearing path is about 3x slower. Returning early when both argument lists are empty, or memoising the string per `FieldNode`, would restore the old cost.

### 2. [Low] Reordered arguments are reported as "differing arguments" when two argument names tie under `naturalCompare`

File: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`, lines 641-649 (interacting with `src/utilities/sortValueNode.ts` lines 44-46 and `src/jsutils/naturalCompare.ts` lines 15-22 and 50).

At base, top-level arguments were matched by exact name (`arguments2.find(...)`), so argument order never mattered. At head, order independence relies on `sortValueNode` putting both argument lists into one canonical order. `sortFields` orders by `naturalCompare`, which parses digit runs into a JavaScript number; two different names whose digit runs differ only beyond 2^53 parse to the same number, and when the names have equal length `naturalCompare` returns 0. The sort is stable, so tied names keep their source order and the two printed strings differ.

Reproduced on head with a schema declaring both arguments:

```graphql
{
  f(a90071992547409920: 1, a90071992547409921: 2)
  f(a90071992547409921: 2, a90071992547409920: 1)
}
```

Base reports no error. Head reports `Fields "f" conflict because they have differing arguments.`, rejecting a valid document. Names with shorter digit runs and names with leading zeros (`a01`, `a1`, `a001`) sort deterministically and are unaffected; I checked those. The same tie already existed at base for fields nested inside input-object values (introduced with `sortValueNode`); this change extends it to top-level arguments. It needs argument names containing a 17+ digit run that differ only in the low digits, which is why it is Low.

## Checked and not reported

- Printing cannot make different argument sets compare equal: string values are quoted and escaped, block strings print differently from plain strings on both base and head, and variables, enums, ints and floats keep distinct spellings. A string containing `", a: "1` is still reported as differing on head.
- Ordinary reordering (`f(a: 1, b: 2)` vs `f(b: 2, a: 1)`), nested input-object reordering, differing values and differing argument counts give the same results on base and head.
- Behaviour differs for fields that repeat an argument name (`f(a: 1, a: 2)`), but such documents are already invalid under `UniqueArgumentNamesRule`, and the head behaviour is the more accurate one.
- The synthetic node has no `loc`; `print` and `sortValueNode` do not need one, and the inputs are copied, not mutated.
- The removed `ArgumentNode` and `ValueNode` imports have no remaining uses in the file. I did not run `tsc` or the linter, because only focused Mocha runs were permitted.
- No developer-experience, configuration or feature-gate impact: the change is confined to one internal function with no exported API change.
