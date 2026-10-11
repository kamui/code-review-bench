# Error contract and tests

## Scope and measurements

Reviewed `src/error/GraphQLError.js`, all three changed error test files, and
the unchanged implementations of `locatedError`, `printError`, `dedent`, and
`invariant`. Consulted `src/language/ast.js` to check the discriminants and
optional object-definition fields used in the new refinements. Reviewed the
base GraphQLError tests with `git show main:src/error/__tests__/GraphQLError-test.js`.

The principal source-evidence commands were `git diff main...review-head`,
`nl -ba` on each changed file, `cat` on those canonical helpers, and targeted
`rg -n` searches for the relevant AST types and stack handling. No ambient
guidance was loaded.

Line counts came from `wc -l` on head files and `git show main:<path> | wc -l`
on their base versions; churn came from `git diff main...review-head --numstat`.

| File | Base lines | Head lines | Added/deleted |
| --- | ---: | ---: | ---: |
| src/error/GraphQLError.js | 220 | 220 | +1/−1 |
| src/error/__tests__/GraphQLError-test.js | 156 | 150 | +27/−33 |
| src/error/__tests__/locatedError-test.js | 46 | 46 | +3/−3 |
| src/error/__tests__/printError-test.js | 96 | 102 | +15/−9 |

No file-size blocker or decomposition requirement arises from this diff.

## Finding: the missing-stack test loses its distinguishing input

At head, GraphQLError-test.js:57–64 still names a case where the original error
has no stack, but line 58 creates an ordinary native Error. On the execution
platform that instance has a nonempty string stack. The constructor at
GraphQLError.js:195 checks `originalError && originalError.stack`, then copies
that stack at lines 196–200. The fallback at lines 201–208 is therefore bypassed.
The assertions check the new error's name, a string stack, message, and original
identity, all of which also hold in the copy branch.

The base fixture was `{ message: 'original' }`, which lacks a stack. It exercised
the distinct condition of a present original error with an absent stack. The
head retains a no-original-error case at lines 34–39, but that does not recover
this distinction. It also already has the explicit stack-reuse case at lines
41–55. This change turns the missing-stack case into duplicate reuse coverage.

This is a test regression introduced by the diff, not an observed regression
in production stack generation. It matters because a future change could fail
specifically for stackless original errors while the named regression case
continues to pass.

## Executed verification

Executed once for this selection and flag set, from the clone:

```sh
BABEL_DISABLE_CACHE=1 ./node_modules/.bin/mocha \
  --require @babel/register --require @babel/polyfill \
  src/error/__tests__/GraphQLError-test.js \
  src/error/__tests__/locatedError-test.js \
  src/error/__tests__/printError-test.js \
  src/jsutils/__tests__/inspect-test.js
```

Result: exit 0, 27 passing, approximately 1.9 seconds wall time. The 13
GraphQLError cases all pass, including the misleadingly named missing-stack
case. The old Browserslist-data notice did not affect execution; no update was
attempted.

A scratch test was written to `../stack-fallback-review-test.js`, relative to
this report directory, and executed once with the same local binary and require
flags. Its absolute path was:

```text
/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-012/clone-work/stack-fallback-review-test.js
```

That scratch experiment first established that the new fixture has a stack and
that GraphQLError copies it verbatim. It then established that an actual Error
whose stack was explicitly cleared gets a newly generated, nonempty stack and
retains original-error identity.

For fault injection, the scratch suite temporarily replaced
`Error.captureStackTrace` with a function that throws only when called by
GraphQLError for an error with a present `originalError`. Calls for errors with
no original were delegated to the original native function. This specifically
breaks the present-original/missing-stack fallback on this Node platform.
Production source was neither edited nor rewritten in memory. An `after` hook
restored the native function.

With that failure injected, the complete head GraphQLError test module still
passed all 13 cases. Two additional scratch assertions verified that both the
base object fixture and the proposed typed, stack-cleared fixture throw the
injected error. The aggregate scratch result was exit 0, 17 passing,
approximately 1.7 seconds wall time. This proves that the changed fixture
removes fault-detection capability for the precise boundary it names.

The fault injection exercises the native-capture branch on the current Node
runtime; it does not claim verification of the alternative runtime path where
`Error.captureStackTrace` is unavailable.

## Worked code-judo remedy

Keep the type-compatible fixture and restore its unusual runtime state directly:

```js
const original = new Error('original');
Object.defineProperty(original, 'stack', { value: undefined });
expect(original.stack).to.equal(undefined);
const e = new GraphQLError('msg', null, null, null, null, original);
expect(e.stack).to.be.a('string');
expect(e.originalError).to.equal(original);
```

Retain the existing name and message assertions as well. This uses an ordinary
property operation to construct a deliberately abnormal native Error. It needs
no widened constructor contract, `any` annotation, generic fixture factory, or
additional production conditional. The key simplification is to model the
boundary state in the fixture rather than substitute a normal state merely to
satisfy its static type. The runtime part of this proposal was verified by the
scratch experiment; its Flow acceptance was not run.

## Other design changes assessed

GraphQLError.js:25 adds `null` to the declared constructor's nodes parameter.
The existing runtime normalization at lines 101–108 already treats null as no
nodes. The newly Flow-enabled tests use null placeholders to reach subsequent
arguments, so this adjustment belongs at the existing constructor boundary.
It adds no fallback or runtime mode. The separately declared implementation
signature at line 94 still omits null; the dual declaration/function arrangement
predates this PR. The diff does not justify rebuilding that arrangement, and
no external typing failure from it was demonstrated.

GraphQLError-test.js:17–26 shares a single parsed source between five location
or serialization cases. It removes repeated parsing and uses the canonical
`invariant` helper to refine the AST's definition union. The literal expected
positions remain independently asserted: field offset 4, operation offset 0,
and explicitly supplied offset 6. No changed test mutates the shared AST, and
the constructor's reductions construct their own arrays. The fixture sharing
does not introduce state coupling in the reviewed cases.

PrintError-test.js:62–80 checks the object-definition kind and optional fields
before accessing field types. The actual error still receives type nodes from
two separate Sources and the expected output retains both source names and
column 10. The added guards are local type refinements, not feature branches in
printing. A source-to-first-field helper could combine the two sequences, but
would hide the two important input Sources behind a new abstraction for one
test. The direct checked extraction is proportionate to this 102-line file;
no dramatic simplification requiring extraction was found.

LocatedError-test.js:29 and 41 introduces two local `any` annotations. These
fixtures intentionally attach foreign GraphQL-like properties or an
Elasticsearch-like string path to native Errors. The canonical locatedError
boundary already performs the cross-context array-path check. The annotations
do not move that policy into test scaffolding or add production bypasses.
A typed `Object.assign(new Error(...), properties)` alternative could model the
augmented objects without these annotations, but the current deliberate foreign
fixtures do not establish a substantive new architecture or safety regression.
This is an evaluated alternative, not an additional actionable finding.

## Verification limits and integrity

Mocha/Babel execution does not perform Flow checking. The execution allowance
was limited to focused test runs, so Flow and lint were not run. No conclusion
here claims that the new strict annotations or the proposed snippet passed the
static checker. Full-suite behavior and cross-engine stack formatting were not
tested.

Initial and subsequent `git status --porcelain=v1` output was empty;
`git diff --exit-code` returned 0. HEAD stayed
`7e39a122eea9292eeffa6905ffdf8a60c5161cfd`, with tree
`9899ad9ba5a87dbe8e99f6a9e4bae295ef82ede0`. The experiment lived in the work
directory, used no network, and disabled Babel's disk cache.
