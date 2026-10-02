# Located errors, error printing, and inspection tests

## Scope and judgment

Reviewed all changes to `src/error/__tests__/locatedError-test.js`, `src/error/__tests__/printError-test.js`, and `src/jsutils/__tests__/inspect-test.js`, with `src/error/locatedError.js` and the canonical `invariant` and `dedent` implementations as supporting evidence. No additional actionable finding was identified. These files enable Flow strict checking without adding production behavior.

## Located-error boundary fixtures

The two new `any` annotations occur at `src/error/__tests__/locatedError-test.js:29` and `:41`. The first fixture intentionally has an Error prototype and GraphQLError-like properties, testing cross-context shape recognition. The second intentionally has a string `path`, testing rejection of an Elasticsearch-like error. Both runtime object shapes and their existing assertions are unchanged.

The canonical boundary is `src/error/locatedError.js:23–26`, which recognizes GraphQL errors through an array-valued `path` and already uses a local cast for the foreign shape. Applying the normal GraphQLError model to the test inputs would erase the incompatibility those tests need to represent. A new fixture class or a shared foreign-error builder would add concepts to two simple cases. These deliberately loose fixture variables are a limited testing escape hatch rather than an API contract regression. The annotations should not be generalized into production APIs, but this PR does not do so.

The equality assertion used for pass-through is unchanged from the base. Its choice of deep equality rather than identity is not a regression introduced by this typing conversion and is not promoted into a separate review demand.

## AST refinement in error printing

At `src/error/__tests__/printError-test.js:62–80`, the diff names each parsed document and its first definition, checks the `OBJECT_TYPE_DEFINITION` discriminant and presence of `fields`, then checks each selected field. Those refinements express the actual parsed shape before reading `.type`. The existing invariant helper owns this check; no bespoke runtime type utility or type cast was introduced.

The assertions at lines 86–100 continue to protect two separately named sources, `String` and `Int` type locations, and the expected caret positions. Runtime test execution confirms that the added refinements preserve these results. The variable names `opA` and `opB` refer to object type definitions rather than operations, but that is a naming nit, not evidence of architectural drift or a material review finding.

The mirrored extraction sequences each exist once for a distinct document. Factoring them into a generic AST-navigation helper would introduce an additional contract and hide the exact discriminant the fixture assumes. A dramatic simplification is not available here. Keeping the direct refinements follows the skill's preference for boring code and canonical helpers.

## Inspect test suppression

The single `$FlowFixMe` at `src/jsutils/__tests__/inspect-test.js:31` precedes an unchanged `String.raw` expected value. It introduces no runtime expression, helper, or behavior. The suppression is confined to one assertion and does not weaken the production inspect signature. The rest of the suite's fixtures and expectations remain unchanged.

The repository's `.flowconfig` recognizes `$FlowFixMe` as a suppression and pins Flow 0.86.0 through its version section; `package.json` also pins flow-bin 0.86.0. These are source facts, not proof that this suppression is necessary or that Flow passes. The execution allowance did not include a full Flow check, so this review does not claim to have reproduced the underlying type diagnostic. Replacing a short raw string with escaped literal syntax would mostly rearrange quote-escaping complexity. Without a demonstrated broader type-boundary failure, it is not elevated into an actionable finding.

## Measurements and commands

Measurements used `wc -l` on the head files and `git show main:<path> | wc -l` for their base versions. The located-error test stays at 46 lines. The print-error test grows from 96 to 102 lines. The inspect test grows from 77 to 78 lines. None approaches the skill's 1,000-line limit.

Source evidence came from `git diff main...review-head`, `nl -ba` on these files and `src/error/locatedError.js`, and focused `rg` searches for AST discriminant checks, foreign Error fixtures, and suppressions in the relevant test directories. An initial combined search also named a nonexistent `flow-typed` directory, which returned exit status 2 while still yielding matches in the source paths. Subsequent searches omitted that directory. No assumption depends on the missing directory.

The four changed test suites ran together using the clone's own Mocha, `@babel/register`, and `@babel/polyfill`, with Babel caching disabled. There were 27 passing tests: 13 constructor, 3 located-error, 2 print-error, and 9 inspect cases. No suite was rerun with the same flag set. Flow, lint, and the full repository test suite were not executed, and there was no network activity.

## Approval implications

There is no new conditional tangled into a shared runtime path, no duplicated production helper, no extra module layer, and no orchestration or atomicity concern. The explicit AST refinements add a small amount of test setup that earns its keep by exposing the parsed discriminant. The report's sole required correction remains the stackless constructor fixture documented in `01_error_constructor.md`; no additional decomposition or boundary rewrite is proposed for these files.
