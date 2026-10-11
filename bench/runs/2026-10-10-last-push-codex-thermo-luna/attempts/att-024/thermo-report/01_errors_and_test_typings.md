# Error API and test typings

## Scope and evidence

Reviewed the committed range `5384d218539dbb6bb39b25e0b7a5dcdd69ad8a11..7e39a122eea9292eeffa6905ffdf8a60c5161cfd`, using `git diff main...review-head` for the five changed files. The production change in `src/error/GraphQLError.js` adds `null` to the declared `nodes` input type. The runtime constructor already treats falsey `nodes` as absent, and the tests already exercise `null` in this argument position. This makes the declaration reflect the existing call contract without introducing a separate runtime path.

The test changes mark `GraphQLError-test.js`, `locatedError-test.js`, and `printError-test.js` as `@flow strict`, and do the same for `inspect-test.js`. The GraphQLError tests now share one parsed source/AST fixture rather than recreating it across individual tests. The printError test adds explicit AST kind and presence invariants to narrow parsed definitions before accessing typed fields. The two `any` annotations in `locatedError-test.js` are confined to fixtures that deliberately attach GraphQL-like properties to native `Error` instances; this models the arbitrary foreign error shape that the test exercises. The `$FlowFixMe` in `inspect-test.js` is confined to the `String.raw` assertion. These localized escapes do not create a broader type or abstraction boundary problem in the reviewed change.

The changes total 48 insertions and 47 deletions across five files. The touched files total 596 lines after the change (220, 150, 46, 102, and 78 lines respectively); none approaches the skill's 1,000-line threshold. No new condition chains, wrappers, or feature-specific shared-path logic were introduced.

## Code-judo assessment

The useful simplification is already present in the patch: parse a compact shared GraphQL source once and reuse its typed nodes for related cases. That removes repeated setup while keeping each assertion focused. The additional invariants in `printError-test.js` make the required AST assumptions explicit at their use site. No larger restructuring is indicated by this diff; extracting helpers or modules for this amount of fixture setup would add indirection without deleting meaningful complexity.

## Verification status

Ran the four changed test files together with the repository's installed Mocha binary:

`./node_modules/.bin/mocha --require @babel/register --require @babel/polyfill src/error/__tests__/GraphQLError-test.js src/error/__tests__/locatedError-test.js src/error/__tests__/printError-test.js src/jsutils/__tests__/inspect-test.js`

Result: 27 passing. Mocha emitted an outdated Browserslist database notice, but the run completed successfully. A Flow typecheck was not run, so this report verifies runtime tests but does not independently verify the newly enabled static checks.

## Findings

No actionable code-quality findings. The `any` and `$FlowFixMe` occurrences are narrow, test-local accommodations and do not warrant separate review findings under the skill's preference for high-conviction structural feedback.
