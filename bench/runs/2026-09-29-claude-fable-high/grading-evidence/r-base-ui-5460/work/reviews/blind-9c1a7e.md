# Review blind-9c1a7e

### Item 1
Location: packages/react/src/field/control/FieldControl.tsx:114
Claim: A programmatic controlled value change always calls `validation.change`, which in onBlur/onSubmit modes takes the revalidate path and publishes all-valid, wiping an error that blur validation just set.
Consequence: `validationMode="onBlur"`, validate returns 'too short' for fewer than 3 chars, consumer trims on blur (`onBlur={() => setValue(v => v.trim())}`). User types 'a ' and blurs: blur commit marks the field invalid, then the value prop changes to 'a', `useValueChanged` fires `commit(value, true)`, and because `valueMissing` is false it calls `publishAllValid`. Reproduced: head ends with data-valid and no error; base ends with data-invalid and 'too short'. The same happens for any programmatic change on an invalid field (setValue('xy') clears the error though 'xy' is still invalid). NumberFieldInput guards this case with `blockRevalidationRef`; Field.Control has no equivalent.
Fix: —

### Item 2
Location: packages/react/src/field/control/FieldControl.tsx:105
Claim: Resetting a controlled field from code after a submit attempt now runs full validation on the reset value, so a freshly cleared form shows required errors.
Consequence: `<Form onFormSubmit={() => setValue('')}>` with `<Field.Control required value={value} />` in the default onSubmit mode. User types 'hello' and submits successfully; `submitAttemptedRef` is true so `shouldValidateOnChange()` is true, and the reset to '' runs `commit('')` with `markedDirtyRef` still true. Reproduced: head shows data-invalid with 'Constraints not satisfied' on the empty reset form; base stays valid with no error. This is the 'form library reset' case the PR description cites as motivation.
Fix: —

### Item 3
Location: packages/react/src/field/control/FieldControl.tsx:111
Claim: Any programmatic value change is compared against the mount-time baseline, so values loaded asynchronously mark the field dirty and, in onChange mode, show validation errors before the user has touched it.
Consequence: Controlled field mounts with `value=''`, then a fetch resolves and the consumer calls `setValue('loaded')`. Reproduced with `validationMode="onChange"`: head sets data-dirty, calls validate once and renders 'too short' with data-invalid; base leaves dirty false with validate not called. The initial value is captured only once per Field.Root, so the field stays dirty until remount, which breaks 'unsaved changes' logic built on dirty state.
Fix: —

### Item 4
Location: packages/react/src/field/control/FieldControl.tsx:93
Claim: Registering the serialized value changes what `validate` receives on submit and `actionsRef.validate()`, and what `Field.Validity` exposes as `value`/`initialValue`, from the raw controlled value to a string.
Consequence: `<Field.Root validate={v => ...}><Field.Control value={5} /></Field.Root>` inside a Form, then submit. Reproduced: base calls validate with `5` (number) and Field.Validity reports `initialValue: 5`; head calls validate with `'5'` (string) and reports `'5'`. A validator doing `typeof value === 'number'`, `value === 5`, or `Array.isArray(value)` (array values become 'a,b') silently changes result, so previously rejected submits can pass or vice versa.
Fix: —

### Item 5
Location: packages/react/src/field/control/FieldControl.tsx:144
Claim: In controlled mode the field sync moved out of the event handler, so `event.preventBaseUIHandler()` and the `nativeEvent.defaultPrevented` guard (React #9023 workaround) no longer suppress dirty, clearErrors, or validation.
Consequence: `<Field.Control value={value} onChange={e => { e.preventBaseUIHandler(); setValue(e.target.value); }} />` with `validationMode="onChange"`. Reproduced: base leaves dirty false and validate uncalled; head sets data-dirty and calls validate once. Likewise a capture-phase `preventDefault()` on the input event leaves validate uncalled on base but called once on head. The existing 'does not clear errors or validate when change is prevented' test only covers the uncontrolled path.
Fix: —

### Item 6
Location: packages/react/src/field/control/FieldControl.tsx:153
Claim: In uncontrolled mode `details.cancel()` skips clearErrors and validation but `setDirty`/`setFilled` still run and the DOM value has already changed, leaving validity stale against the real value.
Consequence: `<Field.Control defaultValue="abcd" onValueChange={(v, d) => d.cancel()} />` with `validationMode="onChange"` and a validator requiring 3+ chars. User changes the input to 'a'. Reproduced: head has DOM value 'a', data-dirty set, validate never called and no error; base reports 'too short'. The PR body says cancel 'stops the internal handling', but only half of it stops, and the field reports not-invalid for a value that fails validation until blur or submit.
Fix: —

### Item 7
Location: packages/react/src/field/control/FieldControl.tsx:110
Claim: `clearErrors(name)` now runs on every programmatic controlled value change, so server-provided Form errors are dropped when the consumer normalizes or restores values in a later render.
Consequence: Form receives `errors={{ x: 'Server error' }}` from a server response, then the consumer updates the controlled value from code (restoring or normalizing the submitted value) in a subsequent render. Reproduced: head removes the 'Server error' message without any user input; base keeps it. It survives only when errors and value change in the same commit, because the Form's own effect runs after the child's.
Fix: —

### Item 8
Location: packages/react/src/field/control/FieldControl.tsx:106
Claim: `value={null}` is treated as controlled by `onChange` (early return) but as absent by `useValueChanged` (serialized to undefined, early return), so a change to null syncs nothing.
Consequence: Consumer keeps `string | null` state and uses `onValueChange={v => setValue(v === '' ? null : v)}`. User types 'a' then clears the field. Reproduced: head leaves data-dirty and data-filled set with an empty DOM value and never calls validate with ''; base clears dirty and filled and validates ''. With a required field in onChange mode the 'required' error never appears.
Fix: —

### Item 9
Location: packages/react/src/field/control/FieldControl.tsx:105
Claim: Controlled field sync now happens through setState calls inside a layout effect, forcing a second synchronous render for every keystroke in onChange mode and on the first keystroke otherwise.
Consequence: Per the PR's own table, a controlled input with `validationMode="onChange"` goes from 1 to 2 renders per keystroke, re-rendering Field.Root and all its context consumers on the text-input hot path. The added render-count test only covers the default onSubmit steady state. Cheaper alternative: keep user-originated sync in the change handler and use the effect only for changes that did not come from the handler.
Fix: —

### Item 10
Location: packages/react/src/field/control/FieldControl.tsx:148
Claim: The dirty/filled/clearErrors/validate sequence is now duplicated between the `useValueChanged` callback and the uncontrolled `onChange` branch, with different ordering and guards.
Consequence: The effect path runs clearErrors, setDirty, setFilled, validate unconditionally; the handler path runs setDirty and setFilled first and gates clearErrors and validate on `defaultPrevented`/`isCanceled`. The two copies already behave differently (findings 5 and 6) and any future fix must be applied twice. A single helper taking the string value and a should-validate flag would keep them aligned.
Fix: —
