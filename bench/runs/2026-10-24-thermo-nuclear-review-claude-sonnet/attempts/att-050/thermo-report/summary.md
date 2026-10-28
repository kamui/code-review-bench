# Thermo Nuclear Review: graphql-js #3457

Scope: `git diff main...review-head`, which changes only `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`.

No actionable findings.

What I checked:
- The change replaces `sameArguments` and `stringifyValue` with `stringifyArguments`. It wraps the field's arguments in a synthetic `ObjectValueNode`, passes that through `sortValueNode`, and prints it. Arguments are therefore compared as a name-sorted object, in the same way that nested object values already were.
- For distinct argument names, the new check gives the same result as the old one. Both compare the argument sets regardless of order and compare each value with object fields sorted. Differing counts or names print differently, so they are reported as differing arguments, as before.
- Duplicate argument names are the only case where the results differ, and the new behaviour is stricter. The old `find` matched the first same-named argument, so `(a:1,a:1)` against `(a:1,a:2)` compared as equal in one direction. Duplicate arguments are already rejected by `UniqueArgumentNamesRule`, so this is not a regression.
- The `FIXME`/`c8 ignore` handling of a missing `arguments` array was moved into the new helper and still defaults to `[]`. The unused imports (`ArgumentNode`, `ValueNode`) were removed and `ObjectValueNode` was added. No other code references the removed helpers.
- `sortValueNode` is not mutated; it returns new nodes. Printing the synthetic node with no `loc` is safe.
- Running `mocha src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts` gave 46 passing, 0 failing.
- The change adds no security surface, devex impact or feature-gate exposure.
