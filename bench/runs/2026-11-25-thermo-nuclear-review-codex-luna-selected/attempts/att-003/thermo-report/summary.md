No actionable findings.

The argument comparison now constructs an object value from the field arguments, then uses the existing value sorter and printer. This preserves order-insensitive comparison of argument names and input object fields, while preserving list ordering. The focused `OverlappingFieldsCanBeMergedRule` test suite passed (46 tests).
