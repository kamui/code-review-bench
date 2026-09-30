# Thermo-nuclear code quality review — `mui/base-ui#5460`

"[field] Sync controlled value changes with field state" · range `30b8ea200..14d39e5d1` · 2 files, +197 / −26

## Verdict

**Changes requested (non-blocking on size; blocking on structure).** The direction is right. Moving `Field.Control`'s controlled
mode onto `useValueChanged` brings it in line with Number Field, OTP Field, Checkbox, and the other sibling controls. It also fixes
two real bugs, which were reproduced in a scratch harness against `main`: numeric dirty state never cleared, and programmatic
changes left dirty/validation stale. The file stays at 207 lines, and the existing suite passes (26 passed, 1 skipped).

The restructure stops short of the simplification it describes, though. The PR body says each mode now has "one owner". In the
source there are still two hand-written copies of the same field-sync sequence, and they already differ in ordering and comments.
Separately, the PR adds a serialized value to the field registration when that registration already reads the same string from the
DOM, so a delete was available where the PR chose an addition. Both code-judo moves were applied in a scratch variant, which
behaved identically to the PR on every probe. Finally, splitting the path in two silently gave the two modes different semantics
for prevented or canceled changes, and that policy is not written down anywhere.

## Findings

### Finding 1 — The field-sync sequence is duplicated across the two mode owners (missed code-judo, verified)

In `packages/react/src/field/control/FieldControl.tsx`, the `useValueChanged` callback (lines 105-115) and the uncontrolled
`onChange` tail (lines 148-156) each perform the same four steps: set dirty against `validityData.initialValue ?? ''`, set filled
against `''`, clear form errors, and call `validation.change`. The PR presents this as "one owner per mode". What actually shipped
is two copies of one owner, and the copies already drift. The controlled copy clears errors before updating dirty, and the
uncontrolled copy does it after. The ordering comment ("`validation.change` reads `markedDirtyRef`, so update dirty before
validating") appears only on the uncontrolled copy, even though both copies depend on that invariant. Any future change to what
dirty or filled means for a text input would have to be made twice, and missing one copy recreates the mode-dependent drift this
PR exists to remove. The remedy is to name the two concepts once. `syncFieldState(value)` owns dirty and filled and always
follows the value. `revalidate(value)` owns error clearing and `validation.change`, and it is the part interception may skip. The
two owners then differ only in when they fire. A worked version, verified to behave identically, is in
`01_field-control.md` (Finding 1).

### Finding 2 — Registering `serializedValue` is redundant; register nothing and let the DOM be the single source (missed code-judo, verified)

`FieldControl.tsx:85-97` builds `serializedValue = String(value)` and registers it with the field, with the comment that the DOM
value is always a string. But `Field.Control` already passes `getValueFromInput` as the registration's `getValue` override.
`useFieldControlRegistration.ts:48-50` falls back to that getter whenever the registered value is `undefined`, and the
uncontrolled mode already relies on that fallback. By the time the registration layout effect runs, React has already written
the controlled value into the DOM, so the getter returns exactly the string the PR constructs by hand. The PR fixes the
`"5" !== 5` baseline bug by adding a second value source that has to agree with the first. It could have deleted the second
source instead. Registering `undefined` in both modes makes the baseline and form-validation value come from one place,
removes the mode-specific registration shape, and stops the registration effect from rebuilding the form's field entry on every
controlled keystroke (its deps include `value`). In the scratch variant, this change reproduced the PR's numeric-dirty fix and
its string-typed Form-submit `validate` argument exactly. `serializedValue` would then serve only as the `useValueChanged`
trigger. Evidence and the diff are in `01_field-control.md` (Finding 2).

### Finding 3 — Controlled and uncontrolled modes now treat intercepted changes differently, and the policy is implicit (asymmetry, verified)

Before the PR, the `react#9023` `defaultPrevented` guard applied to every change. Now it lives only in the uncontrolled tail
(`FieldControl.tsx:152-156`), because controlled `onChange` returns at line 144 and `useValueChanged` revalidates whatever the
consumer commits. A scratch probe fires a prevented `input` event on a controlled control that echoes its value: it validates once
on the PR and zero times on `main`, and `clearErrors` now runs on that path. The existing "change is prevented" test covers only
the uncontrolled mode. The new `details.cancel()` handling is narrower than the PR body says. It skips revalidation in uncontrolled
mode but still updates dirty and filled there. In controlled mode, cancel has no meaning beyond the consumer choosing not to
commit. That may be the right policy, but today it emerges from an early return plus a two-flag guard under a comment that now
covers half the cases. The remedy is to state the rule next to the guard: interception suppresses `revalidate` only for
uncontrolled changes, and controlled consumers reject by not committing. Finding 1's split makes that rule readable. Then add a
controlled twin of the prevented-change test so the behaviour is pinned rather than incidental. Details are in
`01_field-control.md` (Finding 3).

### Finding 4 — The controlled render-count test's name and comment overclaim (legibility, verified by measurement)

`FieldControl.test.tsx` adds "renders once per keystroke for controlled input changes", with the comment "The controlled echo
must not schedule a second render per keystroke". The test runs only in the default `onSubmit` mode. In `onChange` mode, the
PR's own performance table and the scratch measurement (`[2, 2, 2, 2]` renders per keystroke) show the second render is
expected. A reader who sees that second render could take it as a broken invariant and "fix" it by moving work back into the
handler, which would bring back the dual sync path. Rename the test to scope it to `onSubmit` validation, reword the comment to
say the `onChange`-mode render is the accepted cost of `useValueChanged`, and optionally pin that cost with an explicit
assertion. Details are in `02_tests.md`.

## What is good and should stay

- Adopting `useValueChanged` for the controlled mode.
- Removing the `valueProp`-specific branch from the mount-time filled effect. The DOM read covers both modes, and a new test
  pins the empty-controlled case.
- Moving the uncontrolled render counter into the `render` prop. The old wrapper-based counter could not see context-driven
  re-renders of `Field.Control`, so the old test could not catch the regression it was named for.

## Proposed remediation sequence

1. Apply Finding 2 first. Register `undefined` and delete the serialization comment at the registration site. This is a pure
   deletion, and the scratch variant shows it is behaviour-neutral.
2. Apply Finding 1. Extract `syncFieldState` and `revalidate`, route both owners through them, and move the `markedDirtyRef`
   ordering comment onto the helpers.
3. Apply Finding 3. Write down the interception policy next to the `revalidate` guard, and add the controlled-mode
   prevented-change test that pins it (or deliberately choose the other policy and implement it explicitly).
4. Apply Finding 4. Rescope the render-count test's name and comment.

## Verification status

- The existing `FieldControl.test.tsx` passes at `review-head` (26 passed, 1 skipped).
- The scratch harness (`clone-work/scratch/`, 21/21 passing) runs identical probes against `main`'s component, the PR's
  component, and a variant applying Findings 1 and 2. The variant matched the PR on every probe. The asymmetry in Finding 3 and
  the render counts in Finding 4 come from the same run.
- Browser-only behaviour (the chromium SSR `autoFocus` test, and whether a real browser dispatches a cancelable `input` event)
  was not exercised, because browsers are unavailable in this environment.

## Detail files

- `01_field-control.md`: component analysis, the probe table, worked diffs for Findings 1 and 2, and the Finding 3 evidence.
- `02_tests.md`: test-file review, Finding 4, and the coverage gap tied to Finding 3.
