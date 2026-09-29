# Review blind-b6fc40

### Item 1
Location: packages/react/src/field/control/FieldControl.tsx:105
Claim: When Field.Control is rendered as a Combobox input (`<Combobox.Input render={<Input />}>`), the new value-prop sync validates the input's label text on every programmatic input change, competing with Combobox's own validation of the selected value.
Consequence: Field.Root validationMode="onChange" with a controlled Combobox of object items and a field-aware Input; the app calls setValue(items[0]). Scratch run on head: validate is called with [{label:'France',value:'fr'}, 'France'], so the label string is validated last and decides the verdict; base calls validate once with the object. On a user click the order is ['France', object], so a validator expecting an object still gets a string. Autocomplete now validates twice per programmatic change.
Fix: —

### Item 2
Location: packages/react/src/field/control/FieldControl.tsx:114
Claim: Programmatic resets of a controlled value now run full validation, so clearing a required field from code after a submit immediately shows a required error on the fresh form.
Consequence: Form with onFormSubmit={() => setValue('')} and a controlled required Field.Control. User types 'abc' and submits. submitAttemptedRef stays true and markedDirtyRef is never reset, so the reset to '' validates and reports valueMissing. Scratch run on head: root has data-invalid and Field.Error shows 'Constraints not satisfied'; base shows data-valid and no error.
Fix: —

### Item 3
Location: packages/react/src/field/control/FieldControl.tsx:144
Claim: The early return for controlled inputs bypasses the `defaultPrevented` guard (the React #9023 workaround), so a prevented input event still clears form errors and validates once the consumer echoes the value.
Consequence: Form errors={{message:'Server error'}} (stable object), Field.Root validationMode="onChange", controlled Field.Control with onValueChange={setValue}; a capture listener calls preventDefault on the input event. Scratch run on head: validate is called once and the server error disappears; base makes 0 validate calls and keeps the error. The existing 'does not clear errors or validate when change is prevented' test only covers uncontrolled.
Fix: —

### Item 4
Location: packages/react/src/field/control/FieldControl.tsx:144
Claim: `value={null}` counts as controlled (`valueProp !== undefined`), so onChange returns early, while `serializedValue` is undefined and the value-sync callback also returns early; the field gets no state updates at all.
Consequence: <Field.Control value={maybeNull} onValueChange={...}> with a null value, which React treats as uncontrolled so the user can type. Scratch run on head after typing 'abc': no data-dirty, no data-filled, 0 validate calls; base sets dirty and filled and validates once. Likewise a controlled value going from 'abc' to null leaves filled, dirty, and validity stale.
Fix: —

### Item 5
Location: packages/react/src/field/control/FieldControl.tsx:153
Claim: In uncontrolled mode `details.cancel()` skips clearErrors and validation but cannot revert the DOM value (and dirty/filled still update), leaving validity stale for a value that is no longer in the input.
Consequence: Uncontrolled Field.Control, validationMode="onChange", validate rejects 'bad', onValueChange cancels for 'good'. Type 'bad' then 'good'. Scratch run on head: DOM value is 'good' but the root stays data-invalid with the old error shown; base revalidates to data-valid. In controlled mode cancel() has no effect at all if the consumer still sets state, so the PR's cancel semantics are inconsistent between modes.
Fix: —

### Item 6
Location: packages/react/src/field/control/FieldControl.tsx:99
Claim: The mount effect now only ever sets filled to true; the removed branch that reset filled to false for an empty controlled value on mount is not re-established.
Consequence: Inside one Field.Root, swap <Field.Control key="a" value="hello"> for <Field.Control key="b" value="">. Scratch run on head: data-filled stays set on the root after the swap to the empty control; base clears it. The mount effect also no longer falls back to the value prop, so a controlled non-empty value on a rendered element without a DOM `.value` is not marked filled.
Fix: —

### Item 7
Location: packages/react/src/field/control/FieldControl.tsx:110
Claim: Any change to the controlled value prop is now treated like a user edit: it clears Form server errors, marks the field dirty against the mount-time baseline, and validates.
Consequence: value={data?.name ?? ''} where data loads asynchronously, or a form library reset to new defaults, with validationMode="onChange". Scratch run on head after a programmatic setValue('bad'): data-dirty and data-invalid are set and the error shows before the user has touched the field; base only sets data-filled. A server error for that field set before the value update is also cleared.
Fix: —

### Item 8
Location: packages/react/src/field/control/FieldControl.tsx:93
Claim: Registering the serialized string instead of the raw controlled value changes the type of the public `initialValue` and of the value passed to `validate` on submit or `actionsRef.validate()`.
Consequence: <Field.Control value={5}>. Scratch run: Field.Validity reports initialValue '5' (string) on head versus 5 (number) on base. On form submit, `validate` now receives '5' instead of 5, so a validator using `typeof value === 'number'` or strict equality changes its result.
Fix: —

### Item 9
Location: packages/react/src/field/control/FieldControl.tsx:86
Claim: `String(value)` is a lossy serialization for array values: distinct arrays can serialize identically, and the result does not match what the DOM control reports.
Consequence: Controlled `render={<select multiple />}` with value ['a','b'] serializes to 'a,b', which is what validate and validityData.value receive, while select.value (used on blur and Enter commits) is 'a'. ['a,b'] and ['a','b'] serialize the same, so switching between them fires no sync and dirty/validity stay stale.
Fix: —

### Item 10
Location: packages/react/src/field/control/FieldControl.tsx:105
Claim: Moving controlled sync into a layout effect adds a second render per keystroke in onChange validation mode, and the dirty/filled/clearErrors/validate block is now duplicated between the effect and onChange.
Consequence: A controlled input with validationMode="onChange" renders Field.Root and all its consumers twice per keystroke (the PR's own table: 1 to 2). The new 'renders once per keystroke' test only measures onSubmit steady state, so its claim that the echo schedules no second render is unguarded in onChange mode. The two copies of the sync block can drift; a single shared helper called from both paths would avoid that.
Fix: —
