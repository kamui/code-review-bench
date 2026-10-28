# Thermo Nuclear Review: graphql/graphql-js#3457

Range: `730d5af8e933235fd5aa312a00be465db0b8acf5..efdbbcfacd92adb78e002f2b8a61f2a6a1504c19` ("OverlappingFieldsCanBeMergedRule: simplify argument comparison")

## Scope and what changed

The change touches only `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`. The old code paired arguments by exact name (`sameArguments`) and compared each pair's printed, sorted value. The new `stringifyArguments` wraps the argument list in a synthetic `ObjectValueNode` and prints it through `sortValueNode`. The arguments are now ordered by `naturalCompare` on their names, and the two printed strings are compared directly.

## Verification performed

- I read `sortValueNode` (`src/utilities/sortValueNode.ts`), `naturalCompare` (`src/jsutils/naturalCompare.ts`), and the `ObjectValue`/`ObjectField` printers (`src/language/printer.ts:119-120`).
- I ran `./node_modules/.bin/mocha src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts` at head: all 46 tests pass.
- I ran a scratch harness in the work directory against both the head rule and a transpiled copy of the base rule:
  - `{ f(a: 1, b: 2) f(b: 2, a: 1) }`: neither version reports a conflict. The behaviour is unchanged.
  - `{ f(a: """x""") f(a: "x") }`: both versions report a conflict. The behaviour is unchanged.
  - `{ f(a: 1, a: 1) f(a: 1, b: 1) }`: the base reports nothing, and head reports "differing arguments". The old one-directional `find` matching missed this conflict. Head is now symmetric and more correct, so this is an improvement, not a regression.
  - `{ f(a99999999999999999: 1, a99999999999999998: 2) f(a99999999999999998: 2, a99999999999999999: 1) }`: the base reports nothing, and head reports "differing arguments". This is the finding below.
- Collisions from joining the arguments into one string are not possible. The printed form is valid GraphQL, strings are escaped, and `"""` inside block strings is escaped too, so two different argument lists cannot print to the same text.
- The synthetic node has no `loc`, which is harmless for `print` and `sortValueNode`.
- The `c8 ignore` / FIXME handling of missing `arguments` is preserved.
- The import changes are consistent: `ArgumentNode` and `ValueNode` are removed and `ObjectValueNode` is added. Nothing else uses the removed helpers (`sameArguments` and `stringifyValue` were module-private).
- The PR has no devex, security or feature-gate impact.

## Findings

### [Low] Arguments that `naturalCompare` ranks as equal are no longer matched by name, so a valid query can be falsely rejected

- **File/lines:** `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts:591` (call site) and `:637-650` (`stringifyArguments`). The ordering comes from `sortFields` in `src/utilities/sortValueNode.ts` and `naturalCompare` in `src/jsutils/naturalCompare.ts`.
- **Evidence:** The old code matched arguments by exact name (`argument.name.value === argument1.name.value`), so argument order never mattered. The new code relies on sorting with `naturalCompare` and then comparing printed text positionally. `naturalCompare` adds up digit runs as JS numbers (`aNum = aNum * 10 + ...`). Above 2^53, two different digit runs can round to the same number. `naturalCompare` then returns `0` for two different names of equal length, such as `a99999999999999999` and `a99999999999999998`. `Array.prototype.sort` is stable, so those arguments keep their source order, and the same arguments written in a different order print differently.
- **Failure:** With a schema field `f(a99999999999999999: Int, a99999999999999998: Int): Int`, the valid query `{ f(a99999999999999999: 1, a99999999999999998: 2) f(a99999999999999998: 2, a99999999999999999: 1) }` passes at base. At head, the harness confirmed it fails with `Fields "f" conflict because they have differing arguments`. This is a false validation error on a valid operation: argument order is not significant in GraphQL.
- **Priority rationale:** It is a real behavioural regression introduced by this diff for arguments. Input-object fields already had this weakness from #3455. It only triggers when names contain digit runs of more than 16 digits, so its impact in practice is very small; it is reported as Low.
- **Possible fix:** Use a total order for sorting, for example by falling back to plain string comparison when `naturalCompare` returns 0. This would fix arguments and input-object fields together.

No other actionable findings.
