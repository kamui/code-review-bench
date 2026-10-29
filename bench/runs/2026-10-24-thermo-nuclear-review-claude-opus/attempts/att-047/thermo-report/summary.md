# Thermo Nuclear Review: graphql/graphql-js#3457

Range: `730d5af8e933235fd5aa312a00be465db0b8acf5..efdbbcfacd92adb78e002f2b8a61f2a6a1504c19`
Changed file: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`

No actionable findings.

## What changed

`sameArguments(args1, args2)` and `stringifyValue` are replaced by `stringifyArguments(fieldNode)`. The new helper wraps a field's arguments in a synthetic `ObjectValueNode`, sorts it with `sortValueNode`, and prints it. `findConflict` (line 591) now compares the two strings. The `?? []` fallback for `arguments` (FIXME #2203) moves into the helper.

## What I checked

- **Equivalence for valid documents.** The old code needed equal argument counts, and for each argument in the first field it looked up the first argument with the same name in the second field, then compared `print(sortValueNode(value))`. The new code sorts the arguments by name with `naturalCompare`, applies `sortValueNode` to each value recursively, and prints `{name: value, ...}`. When argument names are unique, which `UniqueArgumentNamesRule` enforces, both versions treat the same argument sets as equal regardless of order. The value printing is the same `print(sortValueNode(...))` path as before, so nothing changes for strings, block strings, variables, enums, lists or nested objects.
- **Ambiguity of the printed form.** The printer emits `'{' + join(fields, ', ') + '}'` and `name + ': ' + value` (`src/language/printer.ts:119-120`). Names are GraphQL `Name` tokens and values are printed in GraphQL syntax, so different argument sets cannot print to the same string. With no arguments, or with `arguments` undefined, both fields print `{}`, which matches the old behaviour (empty arrays were equal).
- **Duplicate argument names (invalid documents).** The old code could wrongly call `f(a: 1, a: 1)` and `f(a: 1, b: 2)` equal, because both lookups landed on `a: 1`. The new code prints `{a: 1, a: 1}` and `{a: 1, b: 2}` and reports them as different. `Array.prototype.sort` is stable, so `f(a: 1, a: 2)` against `f(a: 2, a: 1)` still counts as differing. The change only makes the rule stricter, and only for documents that another rule already rejects. No false negatives are introduced.
- **Synthetic node shape.** The node built in the helper has `kind`, `fields` and `name`/`value`, and no `loc`. The printer and `sortValueNode` only read those properties, so leaving out `loc` is safe. Reusing `argNode.name` (a `NameNode`) as the `ObjectFieldNode` name type-checks: `tsc --noEmit` passed cleanly.
- **Public API / breakage.** `sameArguments` and `stringifyValue` were private to the module, and nothing else imports them. The type-only imports `ArgumentNode` and `ValueNode` were removed and `ObjectValueNode` was added. No exports, environment, scripts or feature gates were touched.
- **Performance.** The old early exit on differing argument counts is gone, so both sides are always printed. The cost is linear in argument size and only paid for fields with the same response name that `findConflict` reaches. It is not a meaningful regression.
- **Tests.** `mocha src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts src/utilities/__tests__/sortValueNode-test.ts` gave 48 passing tests, and `tsc --noEmit` reported no errors.
