No actionable findings.

The diff replaces the pairwise `sameArguments`/`stringifyValue` with `stringifyArguments`, which wraps a field's arguments in a synthetic `ObjectValueNode`, sorts it with `sortValueNode`, and prints it. I traced it as follows:
- Argument order is normalized by `sortFields`, and nested objects and lists are sorted recursively. Count and name mismatches show up as differing printed strings, so the old length and lookup checks are covered.
- The only behavioral difference is for duplicate argument names, which `UniqueArgumentNamesRule` already rejects.
- The `node.arguments ?? []` FIXME fallback and its c8 ignore were moved intact.
- The `OverlappingFieldsCanBeMergedRule` test file passes (46 tests).
