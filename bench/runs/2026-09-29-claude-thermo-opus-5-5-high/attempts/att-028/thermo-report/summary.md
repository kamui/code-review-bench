# Thermo-nuclear code quality review — `mui/base-ui#5460`

"[field] Sync controlled value changes with field state". Range
`30b8ea200..14d39e5d1` (`git diff main...review-head`). The PR changes 2 files:
`packages/react/src/field/control/FieldControl.tsx` (179 → 207 lines) and its test file.

## Verdict

The PR moves in the right structural direction, but it stops halfway, and that half-step changes
behavior in two places the PR does not announce. Moving controlled sync onto `useValueChanged`
gives `Field.Control` the same shape as Switch, Checkbox, Radio Group, Select, OTP Field and
Number Field, and it deletes the old `valueProp`-keyed filled effect. That is a real reduction in
competing sync paths, and the file stays small. However, "one owner per mode" was implemented by
copying the field-sync policy into both owners rather than sharing it. The copies already disagree
on ordering and gating, and that disagreement changes behavior in controlled mode. There is no
file-size, layering or cast problem. Recommendation: **request changes** (as a follow-up, since the
PR has merged). Collapse the duplicated policy into one helper, decide the controlled-mode gate
semantics explicitly, and pin the new `validate` contract with tests.

## Findings

### 1. The field-sync policy is duplicated across the two new owners, and the copies already drift

`FieldControl.tsx:105-115` (the controlled `useValueChanged` owner) and `FieldControl.tsx:148-156`
(the uncontrolled `onChange` owner) each compute dirty as
`x !== (validityData.initialValue ?? '')` and filled as `x !== ''`, then call `clearErrors` and
`validation.change`. They do this in a different order and behind different gates. The ordering
rule that the file documents ("`validation.change` reads `markedDirtyRef`, so update dirty before
validating") now has to hold in two places, and only one of them carries the comment. The mode
split itself is expressed as two unrelated ad-hoc guards (a mid-handler `if (isControlled) return;`
and a `serializedValue === undefined` sentinel in the effect), so the reader has to rebuild the
ownership model from scattered conditionals. The code-judo move is to extract one local
`syncFieldValue(nextValue, revalidate)` that owns dirty, filled, the ordering comment and the
`clearErrors` + `validation.change` pair. The effect then calls it with `true`, and the
uncontrolled branch calls it with `!defaultPrevented && !details.isCanceled`. Behavior is
preserved because the setters are batched in both contexts. The gating difference becomes a single
visible argument instead of a structural fork, which makes Finding 2 impossible to miss. The worked
proposal is in `01_field-control-sync.md`, Finding 1.1.

### 2. The react#9023 `defaultPrevented` gate silently stopped applying to controlled inputs

Before this PR, `onChange` skipped `clearErrors` and `validation.change` in both modes when the
native event was default-prevented. The gate now sits after the controlled early return
(`FieldControl.tsx:142-156`), and the controlled owner in `useValueChanged` cannot see the event.
As a result, a prevented input whose consumer still commits the value now clears errors and
validates. A base-vs-head scratch probe of the existing prevented-input scenario, run with a
controlled control, recorded 0 `validate` calls at base and 1 at head. The existing test only
covers an uncontrolled control, so CI stays green. `details.cancel()` has the same asymmetry: in
controlled mode it only stops the internal handling if the consumer also declines to set state. The
PR body mentions neither. Make the controlled-mode semantics an explicit decision. Either document
next to the gate that controlled values are gated by the consumer's commit, or route the gate
through the shared helper's `revalidate` argument. Do not reintroduce a one-shot "skip next sync"
ref. Pin the chosen semantics with a controlled variant of the prevented-input test. Evidence and
options are in `01_field-control-sync.md`, Finding 1.2.

### 3. Serializing the registration value changes what `validate` receives on submit

The PR presents `String(value)` registration (`FieldControl.tsx:85-97`) as a dirty-baseline fix.
But `registration.value` is also what `useFieldControlRegistration`'s `validate()` commits on form
submit (`useFieldControlRegistration.ts:48-62`), so for `<Field.Control value={5}>` the
submit-time `validate` argument changed from the number `5` (base) to the string `"5"` (head), as
the scratch probe confirmed. The new behavior is arguably the coherent one, since the `onChange`
and `onBlur` paths already pass DOM strings. But it is an unannounced change to a public callback
typed `unknown`, and it reaches `<Input>` too. Make the boundary deliberate: rename
`serializedValue` to reflect that it is the field's canonical value in every path, widen the
comment to say so, add a test that pins the submit-time argument, and note it in the release notes.
Details are in `01_field-control-sync.md`, Finding 1.3.

### 4. The new render-count test is named as a general invariant but only holds in `onSubmit` mode

`renders once per keystroke for controlled input changes` runs under the default validation mode
and says "the controlled echo must not schedule a second render per keystroke." The PR body's own
table, and a scratch probe (head `2 2 2` vs base `1 1 1`), show two renders per keystroke in
`onChange` mode. Rename the test to scope it to `onSubmit`, or better, parameterize it over
`validationMode` with the expected per-keystroke counts so the cost the PR accepts is pinned.
Details are in `02_tests-and-verification.md`, Finding 2.1.

### 5. Claimed and changed behaviors lack tests

The PR body's claim that a stale async validator can no longer publish after a programmatic change
has no test, since every new test uses a synchronous `validate`. The behavior changes in Findings 2
and 3 are also unpinned. Add three tests: an async-validator race across a programmatic controlled
change, a controlled prevented-input case, and a submit with a numeric controlled value. Details
are in `02_tests-and-verification.md`, Finding 2.2.

## What is good

The restructuring deletes the old `valueProp`-keyed filled effect and its `valueProp === ''`
special case. Mount-time filled now comes from a single DOM read, and subsequent changes come from
the controlled owner, so the '' case falls out naturally. Comparing serialized strings means a
`5 → '5'` type-only change correctly triggers no revalidation. The `value={null}` edge keeps base
parity (verified).

## Proposed remediation sequence

1. Extract `syncFieldValue(nextValue, revalidate)` in `FieldControl.tsx` and route both owners
   through it (Finding 1). This is a pure refactor with no behavior change.
2. With the gate now a single argument, decide the controlled-mode `defaultPrevented`/`cancel()`
   semantics and encode them there, with a comment (Finding 2).
3. Rename `serializedValue` to express that it is the field's canonical value, and widen its
   comment (Finding 3).
4. Add the controlled prevented-input test, the submit-time `validate` argument test and the
   async-validator race test. Scope or parameterize the render-count test (Findings 2–5).
5. Record the `validate` argument change in the release notes.

## Detail files

- `01_field-control-sync.md` — the structural review of `FieldControl.tsx`, worked code-judo
  proposal, base-vs-head evidence and non-findings checked.
- `02_tests-and-verification.md` — test-suite findings, exact commands and the full probe table.
