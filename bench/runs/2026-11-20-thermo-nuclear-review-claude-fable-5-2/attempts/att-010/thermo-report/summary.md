# Thermo-nuclear review: mui/base-ui#5460 "[field] Sync controlled value changes with field state"

Range reviewed: `30b8ea2004fa999bed151204208676c6c0a9d261..14d39e5d1ad6b7aca2fb067415dba09c6bea219b` (`git diff main...review-head`), two files: `packages/react/src/field/control/FieldControl.tsx` and its test.

Six actionable findings (three Medium, three Low) and one question. No security, devex, or feature-gate issues: the change is confined to one component and its test, adds no dependency, script, environment variable, or flag.

How the findings were verified: every behaviour below was reproduced with scratch jsdom tests run twice, once against the head sources and once against a copy of the merge-base sources extracted into the work directory, so each "head" versus "main" result is observed, not inferred. The existing suites for `field`, `form`, `input`, and `fieldset` pass on head (303 passed, 9 skipped). Nothing was run in a browser, so nothing here depends on rendered output.

What the change does, in short: for a controlled `Field.Control` (and therefore `Input`, which is a thin wrapper), `onChange` now returns right after calling `onValueChange` (lines 142-146), and all field-state work (clear form errors, dirty, filled, `validation.change`) moves to a `useValueChanged` layout effect keyed on the stringified `value` prop (lines 105-115). The findings come from two consequences of that: the effect cannot tell a user edit from a programmatic one, and the `onChange` gate assumes every edit is echoed back through the `value` prop.

## Findings

### 1. [Medium] Normalizing a controlled value on blur erases the blur validation result

`packages/react/src/field/control/FieldControl.tsx`, lines 105-115 (interacting with `packages/react/src/field/root/useFieldValidation.ts`, lines 169-183 and 316-328).

With `validationMode="onBlur"`, a consumer that formats the controlled value in its blur handler (trim, uppercase, phone or currency formatting) no longer gets a blur validation result. The field reads valid while holding a value that fails validation.

Trace: the blur handler runs `validation.commit(event.currentTarget.value)` (line 166), which publishes the error. The consumer's `setValue(normalized)` lands in the same batch. On the following commit the new `useValueChanged` effect sees a different `serializedValue` and calls `validation.change(serializedValue)` (line 114). Outside on-change validation, `change` calls `commit(value, true)`, the "revalidate" shortcut: when the field is currently invalid and the native state is not `valueMissing`, it calls `publishAllValid`, which clears the custom validity and sets the field valid without running `validate`. That shortcut is designed for a user who is mid-edit and will reach another blur. Here the blur has already happened, so nothing validates again until submit.

Reproduced (head versus main):

- Sync validator `v => String(v).trim().length < 5 ? 'Too short' : null`, `onBlur={() => setValue(v => v.trim())}`, type `"ab "`, blur. Main: `data-invalid`, `aria-invalid="true"`, error "Too short". Head: no `data-invalid`, no `aria-invalid`, no error, value `"ab"`.
- Same with an async validator: head discards the pending result entirely, because the echo's `commit` bumps `validationCommitIdRef` (line 118) before returning early, so the awaited blur validation fails the id check at line 269. Main shows "Too short".
- Native constraint, `pattern="[A-Z]{5}"` with `onBlur={() => setValue(v => v.toUpperCase())}`, type `"ab"`, blur. Head: the input's own `validity.valid` is `false`, yet the field shows no `data-invalid` and no error. Main shows the error.

Impact: in `onBlur` mode, inline validation feedback is silently lost for any controlled input that reformats on blur. A surrounding `Form` still re-validates on submit, so invalid data is not submitted through `Form`, but a standalone `Field.Root` has no later checkpoint. The same applies to any programmatic change made while the field is invalid in `onBlur` mode, or in `onSubmit` mode before the first submit attempt: the error is hidden without the new value being validated.

Direction: the prop-sync path should not use the revalidate shortcut for a change that did not come from the user's edit, or should run a full `commit` when the mode's validation boundary has already passed.

### 2. [Medium] Clearing a controlled value after a successful submit now marks the field invalid

`packages/react/src/field/control/FieldControl.tsx`, lines 105-115.

The common "controlled input, clear it after submit" pattern now leaves the field showing a validation error immediately after a successful submit, in the default `validationMode`.

Trace: `Form`'s `onSubmit` sets `submitAttemptedRef.current = true` (`packages/react/src/form/Form.tsx`, line 112) and nothing ever resets it, so `shouldValidateOnChange()` (`packages/react/src/field/root/FieldRoot.tsx`, lines 87-91) is true for the rest of the form's life. Submit also sets `markedDirtyRef.current = true` (`packages/react/src/internals/field-register-control/useFieldControlRegistration.ts`, line 54), which disables the "don't show `valueMissing` on an untouched field" suppression in `getState`. When the consumer then calls `setValue('')`, the new effect runs `validation.change('')`, which is a full `commit('')`: a `required` field reports `valueMissing`, and a custom `validate` runs against the empty string.

Reproduced (head versus main): `<Form onFormSubmit={() => setValue('')}>` around `<Field.Root name="title"><Field.Control required value={value} onValueChange={setValue} /><Field.Error /></Field.Root>`; type "buy milk", submit.

- Main: after the reset the field has no `data-invalid`, no `aria-invalid`, no error.
- Head: `data-invalid`, `aria-invalid="true"`, and `Field.Error` renders the native required message. The same happens without `required` when `validate={v => v ? null : 'Title is required'}`: head shows "Title is required", main shows nothing.

A related consequence of the same effect, which the new test at `FieldControl.test.tsx` line 142 asserts on purpose, so it is noted and not counted separately: a controlled value populated after mount (an edit form that loads its data in an effect or after a fetch) now comes up with `data-dirty` although the user has not touched it, and with `validationMode="onChange"` it is validated and can show an error before any interaction. Reproduced: head `dirty` true, invalid, error shown; main `dirty` false, valid. The dirty baseline is captured once per `Field.Root` at first registration, so the initial empty string stays the baseline.

Impact: a visible regression for add-item, chat, and comment style forms built on `Input` or `Field.Control`, which previously reset cleanly. The sibling controls (Checkbox, Switch, Radio Group) already behave this way after a programmatic change, so this is consistent with the pattern the PR adopts, but the PR body presents the programmatic sync only as a fix (clear button, form-library reset) and does not mention that a reset now surfaces errors. There is currently no way for a consumer to reset the value without triggering validation, short of remounting `Field.Root`.

### 3. [Medium] Checkbox and radio inputs with a `value` attribute lose all change handling

`packages/react/src/field/control/FieldControl.tsx`, lines 142-146 (gate), with line 83 (`isControlled`) and lines 105-108.

The new early return treats the presence of a `value` prop as proof that every edit will come back as a `value` prop change. That does not hold for `type="checkbox"` and `type="radio"`, where `value` is the constant submit value and the state lives in `checked`. For those inputs the `value` prop never changes, so `useValueChanged` never fires, and `onChange` now skips `clearErrors`, `setDirty`, `setFilled`, and `validation.change`. A native radio essentially always carries `value`, so every native radio rendered through `Field.Control` or `Input` is affected.

Reproduced (head versus main):

- `<Form onFormSubmit={...}><Field.Root name="agree"><Input type="checkbox" value="yes" required /><Field.Error /></Field.Root></Form>`: submit unchecked, then tick the box. Main: the error and `aria-invalid` clear as soon as the box is ticked. Head: the box is checked but the field keeps `data-invalid`, `aria-invalid="true"`, and the error message until the next submit.
- Same checkbox under `<Form errors={{ agree: 'Server error' }}>` with `validationMode="onChange"`: on main, ticking calls `validate` once and clears the server error; on head `validate` is not called and "Server error" stays. Removing the `value` attribute restores the main behaviour on head, which isolates the cause to the gate.
- Two `<Field.Control type="radio" value="...">` in one field with `validationMode="onChange"`: selecting one calls `validate` once on main, zero times on head.

The same gate also disables field state for `value={null}`: `isControlled` is `valueProp !== undefined`, so `null` counts as controlled and `onChange` returns early, while `serializedValue` is `undefined` for `null` and the effect returns early too (lines 106-108). Typing then updates nothing: head gives `validate` 0 calls, not dirty, not filled; main gives 1 call, dirty, filled. React already warns about `value={null}`, so this part is minor, but neither path handles it.

Impact: stale error state and missing on-change validation for native checkbox and radio inputs used through `Field.Control` or `Input`. Submission is not blocked, because `Form` re-validates on submit. No usage of these input types through `Field.Control` exists in the repository's docs or tests, so the exposure is consumers who use the native input types directly.

Direction: gate on whether the edit will be echoed through `value` (for example, skip the early return for checkbox and radio types, or when `serializedValue` is `undefined`), not on the mere presence of the prop.

### 4. [Low] `validate` now receives the stringified value at submit for non-string controlled values

`packages/react/src/field/control/FieldControl.tsx`, lines 85-97.

Registering `serializedValue` instead of `value` fixes the dirty baseline, but the registered value is also what `Form` submit and `actionsRef.validate()` pass to the consumer's `validate` callback (`useFieldControlRegistration.ts`, lines 48-62) and what ends up in `validityData.value` and `validityData.initialValue`. Reproduced with `value={5}` and a submit: main calls `validate(5)` (number), head calls `validate("5")` (string). An array value arrives as `"a,b"` instead of the array.

Impact: a validator written against the raw type, such as `typeof value === 'number'` or `Array.isArray(value)`, passed at submit on main and now fails, which blocks submission in the default `onSubmit` mode. On main the argument was already a string on the change and blur paths, so head is the more consistent behaviour; the issue is that the submit-time contract changed without being called out. Worth a line in the changelog if intended.

### 5. [Low] `data-filled` stays set when a controlled control remounts with an empty value

`packages/react/src/field/control/FieldControl.tsx`, lines 99-103.

The mount effect on main also handled `valueProp === ''` by calling `setFilled(false)`. The new mount effect only ever sets `filled` to true, and `useValueChanged` starts its ref at the mounting value, so it reports no change. A control that is replaced inside the same `Field.Root` by one mounting with `value=""` therefore inherits the previous `filled` state.

Reproduced: `<Field.Control key={k} value={value} />` holding `"abc"`, then bump the key and set the value to `""` in one update. Main: `data-filled` is removed. Head: `data-filled` remains while the input is empty.

Impact: a stale style hook (for example a floating label that stays raised) after a keyed reset or a conditional swap of the control. Narrow, since it needs a remount inside a surviving `Field.Root`.

### 6. [Low] Controlled inputs render twice per keystroke whenever change validation is active

`packages/react/src/field/control/FieldControl.tsx`, lines 105-115; `packages/react/src/field/control/FieldControl.test.tsx`, lines 47-80.

On main the validity update was queued inside the change event and batched with the consumer's `setValue`. On head it is queued from a layout effect after the value has committed, which forces a second synchronous render of the `Field.Root` subtree. Measured with the same `render`-prop counter the new test uses: with `validationMode="onChange"`, two keystrokes cost 4 renders on head and 2 on main; the same 4 versus 2 holds in the default mode after one submit attempt.

The new test "renders once per keystroke for controlled input changes" asserts that "the controlled echo must not schedule a second render per keystroke", but it runs in the default mode before any submit, where `validation.change` is a no-op. The assertion does not hold in the modes where validation actually runs, so the test gives more assurance than the implementation provides.

Impact: a doubled render cost per keystroke for controlled inputs with active change validation. No functional break.

## Question

### Q1. The PR description does not match the head commit in three places. Which is intended?

`packages/react/src/field/control/FieldControl.tsx`, lines 105-115.

- The "Open question" section says this PR does not call `clearErrors(name)` on a controlled change. Head does call it, at line 110.
- The body describes a `processedValueRef` guard that stops the prop echo from validating a keystroke twice. No such ref exists; head instead returns early from `onChange` for controlled values (lines 142-146). Finding 3 follows directly from that substitution.
- The body lists a ride-along fix: "A control that unmounts or becomes disabled while focused no longer latches `data-focused`". Nothing in the two changed files touches focus handling beyond moving the existing `autoFocus` effect, and there is no test for it.

If the description is stale, the first two items are only a documentation matter. The third needs confirming, because a reader of the changelog would expect a fix that this commit does not contain.

## Checked and not reported

- Server errors set in the same update as a controlled value change survive: the control's `clearErrors` runs before `Form`'s own `useValueChanged(externalErrors)` effect, so the new errors win (reproduced, same on main and head).
- `details.cancel()` on an uncontrolled control skips `clearErrors` and validation as the body says; on a controlled control, cancelling without updating the value leaves the field state untouched.
- In controlled mode the `defaultPrevented` guard and `event.preventBaseUIHandler()` no longer keep a value change away from the field state (head validates and clears the server error, main did neither). This is consistent with the stated design that the `value` prop is the source of truth, so it is not raised as a finding.
- Strict-mode double effects, the registration and `useValueChanged` effect order, and the initial-value capture for numeric and array values were traced and behave correctly.
