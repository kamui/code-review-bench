# Review blind-a276cd

### Item 1
Location: packages/react/src/field/control/FieldControl.tsx:144
Claim: Any `value` prop makes the control count as controlled, so native checkbox/radio inputs, whose `value` is a constant submit value, lose all change handling.
Consequence: `<Field.Root name="agree" validationMode="onChange" validate={fn}><Field.Control type="checkbox" value="yes" required /></Field.Root>` inside a Form with a server error: clicking the checkbox returns early from onChange, and useValueChanged never fires because `value` stays "yes". Scratch test confirmed: base calls validate once and clears the error; head calls validate 0 times and the server error stays visible.
Fix: —

### Item 2
Location: packages/react/src/field/control/FieldControl.tsx:105
Claim: The controlled path drops the `event.nativeEvent.defaultPrevented` guard (React #9023 workaround) that used to skip clearErrors and validation for prevented changes.
Consequence: Controlled `<Field.Control value={v} onValueChange={setV} />` with validationMode="onChange" and an input event that is preventDefault-ed: onValueChange still fires, the consumer sets state, and useValueChanged runs clearErrors and validation.change unconditionally. Scratch test confirmed: base validate calls = 0, head = 1. The existing prevented-change test only covers the uncontrolled path.
Fix: —

### Item 3
Location: packages/react/src/field/control/FieldControl.tsx:111
Claim: Every programmatic change to a controlled value is treated as a user edit against the mount-time baseline, so async-loaded or form-library-reset values mark the field dirty and validate immediately.
Consequence: An edit form mounts with `value=""`, then a fetch or `reset(values)` sets value to "loaded" with validationMode="onChange". Scratch test confirmed: base leaves the field pristine with no validate call; head sets data-dirty and data-invalid, calls validate, and renders the error on an untouched field. markedDirtyRef is also flipped, so valueMissing noise suppression is lost.
Fix: —

### Item 4
Location: packages/react/src/field/control/FieldControl.tsx:100
Claim: The mount effect no longer resets `filled` for an empty controlled value, so a remounted control leaves Field.Root with stale `data-filled`.
Consequence: `{show && <Field.Control value={v} .../>}` with v="foo" (filled=true); hide the control and set v to '', then show it again. useValueChanged does not fire on mount and the mount effect only ever sets true. Scratch test confirmed: base data-filled is absent after remount; head still has data-filled with an empty input.
Fix: —

### Item 5
Location: packages/react/src/field/control/FieldControl.tsx:105
Claim: The useValueChanged sync runs for disabled controls, so a programmatic change to a disabled controlled input runs validation and marks the field dirty and invalid.
Consequence: `<Field.Control value={v} disabled />` with validationMode="onChange" and a failing validator; code sets v from "foo" to "bar". Scratch test confirmed: base has 0 validate calls and the field stays pristine; head calls validate and sets data-invalid and data-dirty on the root. A disabled control is unregistered, so initialValue is null and dirty is compared against ''; dirty then stays set after enabling and returning to "foo".
Fix: —

### Item 6
Location: packages/react/src/field/control/FieldControl.tsx:153
Claim: For uncontrolled inputs, `details.cancel()` skips clearErrors and validation but still updates dirty/filled and cannot revert the DOM value, leaving validity stale.
Consequence: Uncontrolled control with validationMode="onChange": typing "bad" gives a validate error; typing "good" while onValueChange calls details.cancel(). Scratch test confirmed: the DOM value is "good" and dirty is updated, but validate is not re-run and the field stays data-invalid with the error for "bad". Base revalidated and cleared it.
Fix: —

### Item 7
Location: packages/react/src/field/control/FieldControl.tsx:93
Claim: Registering the serialized value changes the type passed to `validate` on submit and actionsRef.validate() from the raw prop to a string.
Consequence: `<Field.Root validate={(v) => typeof v === 'number' && v > 3 ? null : 'err'}><Field.Control value={5} /></Field.Root>`: on Form submit or actionsRef.validate(), base passed the number 5 and head passes "5" (scratch test confirmed), so strict type or equality checks in existing validators now fail. Array values arrive as a comma-joined string.
Fix: —

### Item 8
Location: packages/react/src/field/control/FieldControl.tsx:86
Claim: `String(value)` is lossy, and filled/dirty/validation now derive from the prop rather than the DOM value, so they can disagree with what the input actually holds.
Consequence: Not run, from reading the code: `type="number"` with `onValueChange={(v) => setValue(parseFloat(v))}`; the user clears the input, value becomes NaN, serializedValue is "NaN", so filled=true and validate receives "NaN" while the DOM value is ''. Arrays `['a,b']` and `['a','b']` both serialize to "a,b", so a change between them never fires useValueChanged. Mount-time filled reads the DOM while updates read the prop.
Fix: —

### Item 9
Location: packages/react/src/field/control/FieldControl.tsx:114
Claim: Moving controlled validation into a layout effect adds a second synchronous render of the Field subtree per keystroke in onChange mode.
Consequence: Controlled input with validationMode="onChange": each keystroke renders once for the value change, then validation.change in the layout effect calls setValidityData with a new object and re-renders Field.Root and all context consumers. The PR's own table shows 1 render per keystroke on base and 2 on head; the new render-count test only covers onSubmit mode, so this cost is unguarded.
Fix: —
