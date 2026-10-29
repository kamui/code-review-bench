# Thermo Nuclear Review — mui/base-ui#5460 "[field] Sync controlled value changes with field state"

Range reviewed: `30b8ea2004fa999bed151204208676c6c0a9d261..14d39e5d1ad6b7aca2fb067415dba09c6bea219b` (`git diff main...review-head`), 2 files: `packages/react/src/field/control/FieldControl.tsx` and its test file.

## What the change does

`Field.Control` (and therefore `Input`, which is a thin wrapper around it) previously drove dirty, filled, error-clearing and validation entirely from the DOM `onChange` event, and registered the raw `value` prop as the field's dirty baseline. The PR:

- registers `String(value)` (`serializedValue`) instead of the raw prop, so the baseline and the DOM string compare correctly for numeric/array values;
- adds a `useValueChanged(serializedValue, ...)` layout effect that clears Form errors, updates dirty/filled, and calls `validation.change` whenever the controlled prop changes;
- makes the `onChange` handler return early when `isControlled` (`valueProp !== undefined`), so controlled inputs are driven only by the prop path;
- honours `details.cancel()` on the uncontrolled path;
- replaces the mount/prop-driven `setFilled` effect with a mount-only DOM check.

I read the full `FieldControl.tsx`, `useValueChanged`, `useRegisterFieldControl`, `useFieldControlRegistration`, `useFieldValidation`, `FieldRoot`, `Form`, and the sibling `useValueChanged` consumers (Switch, Checkbox, RadioGroup, NumberField, Slider, CheckboxGroup, Combobox). I ran the field, input, fieldset and form jsdom suites from the clone (303 passed, 9 skipped), and ran scratch probes from the work directory against both the head sources and an exported copy of the base sources to compare behaviour.

Verified as correct: single validation per keystroke on the controlled path (the DOM path is skipped, and the prop echo fires exactly once); `setDirty` still runs before `validation.change` so `markedDirtyRef` is set before `commit` reads it; layout-effect ordering means the DOM is already updated when `commit` reads native validity; a rejected change (consumer keeps the old value) now produces no validation and no dirty flip, where base validated the rejected keystroke; a Form `errors` update and a value update in the same render keep the error, because Form's own `useValueChanged` runs after the child's `clearErrors`; form submit values are unchanged (still read from the DOM via `getValueFromInput`).

## Findings

### Finding 1 — Medium — `packages/react/src/field/control/FieldControl.tsx` lines 83–84 and 142–146: a native checkbox or radio rendered through `Field.Control` with a `value` attribute no longer updates field state or revalidates when toggled

**Evidence.** `isControlled` is computed as `valueProp !== undefined` (line 83). For `<input type="checkbox">` and `<input type="radio">` the `value` attribute is the submission value, not a controlled text value; React controls those inputs through `checked`, and `value` stays constant across toggles. With the PR, the `onChange` handler returns at line 144–146 before `setDirty`, `setFilled`, `clearErrors` and `validation.change`, because `isControlled` is true. The prop path at lines 105–115 never fires either, because `serializedValue` ("yes") never changes. So nothing in the field reacts to the toggle.

On the base branch the same markup ran the DOM path: `validation.change("yes")` on every toggle, which in the `onSubmit` revalidation path (`useFieldValidation.commit` with `revalidate = true`) reads `element.validity.valueMissing` and publishes all-valid once the box is checked.

Reproduced with a scratch test (jsdom, same markup on both branches):

```tsx
<Form>
  <Field.Root name="terms">
    <Field.Control type="checkbox" value="yes" required />
    <Field.Error match="valueMissing">Required</Field.Error>
  </Field.Root>
  <button type="submit">go</button>
</Form>
```

Submit with the box unchecked, then click the box:

| | base (`main`) | head (`review-head`) |
| --- | --- | --- |
| "Required" error still shown after checking | no | **yes** |
| `aria-invalid` on the checkbox after checking | removed | **still `"true"`** |

With `validationMode="onChange"` and a custom `validate`, base called `validate` on every toggle; head never calls it. The same markup without the `value` attribute behaves identically on both branches, which isolates the cause to the `isControlled` early return.

**Failure.** Any consumer using `Field.Control` / `Input` as a native checkbox or radio with a submission `value` (a very common HTML pattern, e.g. a terms-of-service checkbox with `value="yes"`) gets a stuck "Required" error and `aria-invalid="true"` after a failed submit even once the user satisfies the constraint, and onChange-mode validation stops responding to the control. This is a regression introduced by the diff, not pre-existing. The guard should distinguish "the `value` prop controls the text value" from "`value` is a constant submission value" (for example, treat `type="checkbox"` / `type="radio"` as not value-controlled, or keep the DOM path for inputs whose `value` did not change in response to the event).

### Finding 2 — Low — `packages/react/src/field/control/FieldControl.tsx` lines 105–115: the pull request body describes a different implementation and misstates the `clearErrors` behaviour

**Evidence.** The body says the guard against double validation is a `processedValueRef` that "records the value the change handler just handled"; the diff has no such ref, and instead skips the DOM path entirely when controlled (lines 142–146). The body says "A control that unmounts or becomes disabled while focused no longer latches `data-focused`"; there is no change to the focus handling anywhere in the diff (the `autoFocus` effect at lines 119–123 is only moved). The body's open question says "The sibling controls also call `clearErrors(name)` when their value changes. This PR does not", but line 110 of the diff calls `clearErrors(name)` on every programmatic controlled change, matching the siblings.

**Failure.** The description is not a code defect, but it is the only statement of intent for a merged change and it is wrong on three points. In particular, the `clearErrors` behaviour it says is absent is present: a programmatic value change on a controlled `Field.Control` now removes a Form-level server error for that field. That is defensible (it matches Switch, Checkbox, etc.), but reviewers and release notes relying on the body will believe the opposite. The body should be corrected to match the merged code, and the `clearErrors` decision should be stated explicitly since it is a behaviour change for `Input`/`Field.Control` consumers that display server errors.

## Notes (not findings)

- `validate()` from form submission or `actionsRef` now receives the serialized string for a numeric/array controlled value (`validate("5", ...)` instead of `validate(5, ...)`). This is what the body says it does ("registers the serialized value") and the change path already passed strings before, so validators now see one consistent type. Mentioned only so it is visible as an intentional behaviour change.
- Native `event.preventDefault()` on the `input` event no longer suppresses `clearErrors`/validation for a controlled input whose consumer then updates the value (the React issue 9023 workaround at line 153 only applies on the uncontrolled path now). Since the consumer did update the value, validating it is at least as correct as before, and a consumer that rejects the value gets no validation at all, which is better than base.
- Loading a value into a controlled `Field.Control` after mount (e.g. `useEffect(() => setValue(fetched))`) now marks the field dirty and, in onChange mode, validates immediately. This matches every sibling control and the documented baseline semantics ("Consumers that want a fresh baseline remount or key `<Field.Root>`"), so it is intended.
- The modified test "avoids rerendering for uncontrolled input changes" now counts renders via the `render` prop instead of a wrapper component; the old wrapper never re-rendered on context changes, so the new form measures what the name claims.
