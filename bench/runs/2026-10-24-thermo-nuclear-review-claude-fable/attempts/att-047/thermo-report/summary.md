# Thermo Nuclear Review: graphql/graphql-js#3457

Range reviewed: `730d5af8e933235fd5aa312a00be465db0b8acf5..efdbbcfacd92adb78e002f2b8a61f2a6a1504c19` (commit "OverlappingFieldsCanBeMergedRule: simplify argument comparison"). One file changed: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`.

## Verdict

No actionable findings.

## What the change does

The commit replaces the `sameArguments` and `stringifyValue` helpers in the overlapping-fields rule with a single `stringifyArguments(fieldNode)` helper. The new helper wraps a field's argument list in a synthetic `ObjectValueNode` (each `ArgumentNode` becomes an `ObjectFieldNode` reusing the original `name` and `value` nodes), sorts it with the existing `sortValueNode` utility, and prints it. The two fields' printed strings are compared with strict inequality to decide whether they "have differing arguments". The `FIXME` for issue 2203 and the `/* c8 ignore next */` marker for the `arguments ?? []` fallback move into the helper unchanged.

## What I verified

- **Semantic equivalence on valid documents.** `sortValueNode` sorts object fields by name with `naturalCompare` and recurses into nested objects and lists, and the printer renders object fields deterministically (`{a: 1, b: 2}`). For any pair of fields with unique argument names, the new comparison yields exactly the same result as the old length-plus-lookup comparison: argument order is ignored, nested input-object field order is ignored, missing or extra arguments conflict, and differing values conflict. I confirmed this with a scratch harness that ran the old and new logic side by side over seventeen cases including empty argument lists, one-sided arguments, reordered arguments, nested reordered input objects, variables, block vs. regular strings, `1.0` vs `1`, enum vs string, and natural-order names such as `a1`/`a10`/`a2`. All of them agreed.
- **Fields with no `arguments` property.** A hand-built `FieldNode` without an `arguments` property (the shape issue 2203 describes) stringifies to `{}` and does not throw, matching the old `?? []` behavior.
- **Only divergence is on already-invalid documents.** The results differ only when a single field repeats an argument name, which `UniqueArgumentNamesRule` independently rejects. Old code treated `f(a: 1, a: 1)` and `f(a: 1, b: 1)` as identical and treated two literally identical `f(a: 1, a: 2)` selections as conflicting. New code gets both of these right. This is a corner-case improvement in invalid input, not a regression, and is not reportable.
- **Synthetic node is safe for `print`.** `visit` only requires `kind` to be a string and walks `QueryDocumentKeys` (`fields` for `OBJECT`, `name`/`value` for `OBJECT_FIELD`); no `loc` is needed. The reused `NameNode` and `ValueNode` are genuine parser output.
- **No other callers were broken.** `sameArguments` and `stringifyValue` were module-private and had no other references. The only other `sortValueNode` consumer (`findBreakingChanges.ts`) is untouched.
- **Toolchain.** `tsc --noEmit` passes, `eslint` passes with zero warnings on the changed file, `prettier --check` passes, and the targeted suites (validation, utilities, language: 1074 tests) and the full `npm run testonly` suite (1860 tests) pass on the head revision. The repository's `c8` coverage run reports 0% for every file under the Node 24 runtime available here, which is a tooling incompatibility unrelated to this change; by inspection the new helper's only branch is the `?? []` fallback, which carries the same coverage-ignore marker as before, and the `map` callback is exercised by the existing argument-conflict tests.
- **Security, devex, feature gates.** The change is a pure refactor inside a validation rule. It touches no environment variables, scripts, ports, secrets, feature flags, or public API surface. The rule still rejects the same set of documents on valid input, so no validation safeguard is weakened.
