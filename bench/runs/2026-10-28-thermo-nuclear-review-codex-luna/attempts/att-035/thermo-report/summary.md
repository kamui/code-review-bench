No actionable findings.

The constructor type change aligns the declared `GraphQLError` API with its existing null-tolerant runtime handling. The test changes preserve the tested behavior while adding strict Flow annotations and explicit AST narrowing. The four affected test files pass (27 tests).
