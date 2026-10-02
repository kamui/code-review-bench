# Verification, measurements, and execution record

## Identity and review constraints

The clone was read at head
`14d39e5d1ad6b7aca2fb067415dba09c6bea219b`. Local main is the recorded merge-base
`30b8ea2004fa999bed151204208676c6c0a9d261`. Only the committed change was reviewed.
No remote, upstream discussion, network resource, ambient guidance, additional
skill, or independent reviewer context was used.

The supplied frozen skill was read from `../clone-work/frozen-skill/SKILL.md`.
It did not require any child calls or reference-resource reads. The review used
one primary context throughout.

Scratch files are in `../clone-work/field-review-tests/`. That directory contains
`vitest.config.mts`, `boundaries.test.tsx`, `observations.test.tsx`,
`prevention-probe.test.tsx`, and `BaseFieldControl.tsx`. Its node_modules symlink
points at the installed `packages/react/node_modules` in the clone. The scratch
config is a plain object with jsdom, local test inclusion, and React/Utils source
aliases, as permitted by the execution policy.

The base comparison extracts `main:packages/react/src/field/control/FieldControl.tsx`
into scratch and rewrites relative import paths to their absolute clone source
paths. Base and head therefore share the same context providers and installed
dependencies. The implementation diff is limited to FieldControl, so this is a
focused control comparison, not a claim to have executed a complete separate
base checkout. The extracted component logic itself is unchanged.

## Measurements

`git diff --numstat main...review-head` reports 48 additions and 20 deletions for
FieldControl.tsx, and 149 additions and six deletions for FieldControl.test.tsx.
`git show main:<path> | wc -l` and `wc -l <head path>` give production sizes of
179 and 207 lines, and test sizes of 130 and 273 lines. The skill's file-size
threshold is not approached.

The primary structural additions are one canonical value observer, one prop
serialization rule, a controlled-event early return, and one cancellation term in
the existing native prevention condition. The last condition protects only the
uncontrolled source. The mount effect changes from a two-direction controlled
projection to a truthy-only DOM read. These observations motivated the targeted
boundary tests rather than broad speculative refactoring.

## Existing unit suites

From the clone root:

```sh
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx
```

Result: one file passed, 26 tests passed, one skipped, 1.54 seconds total. The
skipped test covers browser-focused SSR autofocus behavior. The permitted jsdom
run cannot settle that browser-only case.

The follow-up selection was justified by persistent Field.Root state, Form error
projection, and Input's direct delegation to Field.Control:

```sh
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/root/FieldRoot.test.tsx packages/react/src/form/Form.test.tsx packages/react/src/input/Input.test.tsx
```

Result: three files passed, 139 tests passed, three skipped, 2.96 seconds total.
Input had 15 tests, Form had 48, and Field.Root had 79 with three skipped. Across
all four existing suites, 165 tests passed and four were skipped. No existing
selection was repeated.

## Scratch comparison selection

The commands used the clone's own Vitest binary. The absolute paths below identify
the permitted work directory and installed binary unambiguously:

```sh
TZ=UTC /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-010/clone/node_modules/.bin/vitest run --root /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-010/clone-work/field-review-tests
```

At this execution, only `boundaries.test.tsx` existed as a test selection. It ran
10 cases: five passed and five failed in 1.39 seconds. The failures were deliberate
assertions of the desired boundary behavior, not failures in the target's existing
suite.

For native prevention, head called the validator once when zero calls were
expected; base passed. For empty controlled remount, head retained data-filled
when false was expected; base passed. For a consumer-rewritten range value,
head validated `"200"` while Form values held `"100"`; base passed the same-call
consistency check against its transient event value. That last base result does
not establish correct committed-value handling at base; see the representation
analysis in the field-state detail report.

Uncontrolled cancellation failed at both revisions for different first
assertions. Head skipped validation, then failed because dirty was true. Base
failed because validation was called. Separate observational cases below directly
measured both revisions' dirty and filled flags.

The two head-only protection tests passed: rejecting a controlled change leaves
dirty, filled, and validation unchanged; and a programmatic replacement invalidates
a pending asynchronous result for the old value. The latter observed validation
calls for `old` and `new` and no invalid field state after the old promise resolved.

## Additional observations and correction of an exploratory assertion

A new test file was added only in scratch to measure all cancellation flags and
programmatic range normalization:

```sh
TZ=UTC /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-010/clone/node_modules/.bin/vitest run --root /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-010/clone-work/field-review-tests observations.test.tsx
```

Four cases ran in 0.99 seconds: three passed and one exploratory assertion failed.
The passing cancellation cases measured dirty=true and filled=true at both
revisions, with validator counts zero at head and one at base. The passing
programmatic range case measured DOM `"100"` and validator arguments
`['200', { amount: '100' }]` at head.

The failing case first confirmed one validation call after native prevention,
then expected immediate disappearance of the server-error text. Its App recreated
`errors={{ message: 'Server error' }}` on every controlled render. Form itself
observes externalErrors identity changes and reintroduces that error, so this
fixture could not independently measure internal error clearing. Field.Error can
also retain text while exiting. The failed text assertion is not reported as a
new bug or used as evidence of error removal.

A separate file fixed the fixture by keeping the errors object stable and measured
Form context directly, avoiding rendered transition timing:

```sh
TZ=UTC /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-010/clone/node_modules/.bin/vitest run --root /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-010/clone-work/field-review-tests prevention-probe.test.tsx
```

Both cases passed in 0.89 seconds. The head committed input value `a`, called the
validator once, and exposed Form errors `{}`. The base committed the same input
value, called the validator zero times, and retained the original server error.
This resolves the exploratory fixture issue and confirms the error-clearing
portion of finding F1. No test selection or flag set was repeated.

## Coverage gaps and proposal verification

The changed tests correctly cover numeric dirty reset, one validation per accepted
controlled user change, external controlled changes, fresh-field filled
initialization, and cancellation's validation count. They do not exercise the
controlled native-prevention branch, a reused filled field at control remount, or
a browser-normalized accepted controlled value. These are targeted additions
needed for the three findings, not requests for tests that merely mirror code.

The worked code-judo sketch was checked against the existing registration
contract and effect order by source inspection. It was not applied, compiled as a
replacement, benchmarked, or run through these tests. Its event provenance
mechanism requires implementation and targeted validation. No correctness claim
is made for an unapplied remedy.

Browser execution and rendered visual assessment were unavailable. No full
repository suite, type checker, production render benchmark, or registry/network
command was run. All executed commands completed well within the five-minute
allowance. No additional test run is needed to establish the reported defects.

## Checkout integrity

Initial `git status --short` was empty, and the pinned HEAD matched the packet.
At the end of test execution, `git status --porcelain=v1` was empty,
`git diff --exit-code` succeeded, and `git diff --cached --exit-code` succeeded.
The recorded identities were:

```text
HEAD:       14d39e5d1ad6b7aca2fb067415dba09c6bea219b
HEAD tree:  886c9868c129584e419e29a6aefa273d70bcff39
main tree:  0c3e42e939a6985af9f27385e9cb4d0fe4458faf
```

Reports and scratch tests were written only under clone-work. Installed,
ignored dependencies in the clone were left alone. No remedy, checkout change,
commit, branch mutation, or forge action was performed.
