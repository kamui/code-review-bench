# 01 — `FieldControl.tsx`: the value-sync structure

Scope: `packages/react/src/field/control/FieldControl.tsx` at head `14d39e5d1` against merge-base `30b8ea200` (`git diff main...review-head`). The file is 207 lines after the change, so there is no size concern. Everything below is about how the change is shaped.

## How the evidence was produced

The clone was only read. All execution happened in `clone-work/scratch/`, set up the way the execution policy describes (a `node_modules` link to `packages/react/node_modules`, a plain-object `vitest.config.mts` with the `@base-ui/react/*` and `@base-ui/utils/*` aliases).

Three variants of the control were exercised inside the same head `Field.Root` and `Form`:

- **head** — `Field.Control` straight from the clone.
- **base** — `git show main:packages/react/src/field/control/FieldControl.tsx`, saved as `scratch/BaseFieldControl.tsx` with its relative imports rewritten to absolute clone paths. It shares the head's contexts, so only the control differs.
- **judo** — a copy of the head file with the restructuring proposed below applied (`scratch/JudoFieldControl.tsx`).

Commands:

```
TZ=UTC <clone>/node_modules/.bin/vitest run --root <work>/scratch --reporter=verbose --silent=false
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx
```

The PR's own file passes at head: 26 passed, 1 skipped (the chromium-only SSR test). The scratch run is 57 tests across `probe.test.tsx`, `probe2.test.tsx` and `ported.test.tsx`, all passing; the probes log observations rather than assert, and the numbers quoted below are those logs. Everything ran under jsdom; nothing here was checked in a real browser.

## Finding A — one sync, written twice, with different guards

After the change the control has two places that push a new value into field state.

The controlled path, `FieldControl.tsx:105-115`:

```tsx
useValueChanged(serializedValue, () => {
  if (serializedValue === undefined) {
    return;
  }

  clearErrors(name);
  setDirty(serializedValue !== (validityData.initialValue ?? ''));
  setFilled(serializedValue !== '');

  validation.change(serializedValue);
});
```

The uncontrolled path, `FieldControl.tsx:137-157`:

```tsx
onChange(event) {
  const inputValue = event.currentTarget.value;
  const details = createChangeEventDetails(REASONS.none, event.nativeEvent);
  onValueChange?.(inputValue, details);

  if (isControlled) {
    return;
  }

  // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
  setDirty(inputValue !== (validityData.initialValue ?? ''));
  setFilled(inputValue !== '');

  // Workaround for https://github.com/react/react/issues/9023
  if (!event.nativeEvent.defaultPrevented && !details.isCanceled) {
    clearErrors(name);
    validation.change(inputValue);
  }
},
```

These are the same four operations on the same four collaborators. They differ in three ways, none of which is stated as intentional:

| | controlled (effect) | uncontrolled (handler) |
| --- | --- | --- |
| order | `clearErrors`, `setDirty`, `setFilled`, `change` | `setDirty`, `setFilled`, then `clearErrors`, `change` |
| `details.isCanceled` | ignored | gates `clearErrors` + `change` only |
| `nativeEvent.defaultPrevented` | ignored | gates `clearErrors` + `change` only |

The ordering comment ("update dirty before validating") lives on one copy only, although the constraint binds both.

### The cancel guard leaves the field half-updated

`!details.isCanceled` was appended to a condition whose comment still describes it as a React-bug workaround. Because that condition only wraps the last two steps, a cancelled change on an uncontrolled control updates `dirty` and `filled` and skips error-clearing and validation.

Probe H2 (uncontrolled, `validationMode="onChange"`, validator rejects `'a'`, the handler cancels every other value). Type `a`, then clear the input:

| variant | DOM value | `data-filled` | `data-dirty` | `data-invalid` | error text | `validate` calls |
| --- | --- | --- | --- | --- | --- | --- |
| base | `""` | no | no | no | none | 2 |
| head | `""` | no | no | **yes** | **`bad`** | 1 |

At head the field reports "not filled, not dirty" and at the same time shows the validation error for the `a` that is no longer in the input. The DOM, two of the state flags, and the validity now describe two different values.

Probe H7 (uncontrolled, `validationMode="onChange"`, `validationDebounceTime={100}`, the handler cancels `'ab'`). Type `a`, type `ab`, advance timers:

| variant | DOM value | value passed to `validate` |
| --- | --- | --- |
| base | `ab` | `ab` |
| head | `ab` | **`a`** |

`validation.change` is what clears the pending debounce timer (`useFieldValidation.ts:317`). Skipping it on cancel leaves the timer for the previous keystroke armed, so a validation for the superseded value runs and publishes after the input has moved on. The PR body lists "a pending async validator for the old value could still publish" as a bug this change removes; on this path the change introduces it.

The underlying problem is that an uncontrolled native input cannot be cancelled. By the time `onChange` runs the browser has already committed the keystroke, so `cancel()` cannot mean "the value did not change". There are two coherent answers and the head picked neither:

- **Cancel skips the whole sync.** The field keeps describing the previous value consistently. It is stale against the DOM, but it is not self-contradictory.
- **Cancel does not gate the sync when uncontrolled.** Field state always follows the DOM. `cancel()` is then a no-op for this component, which is what it physically is, and the veto mechanism for a text input is the controlled `value` prop.

### The controlled path dropped both guards without saying so

The early `return` for controlled values is the right idea and the comment explaining it is good. But it also means the two guards no longer exist for controlled controls.

Probe P1 (controlled, `Form` with a stable `errors={{ message: 'Server error' }}`, a capture-phase listener calls `preventDefault()` on the native `input` event — the same setup as the existing "does not clear errors or validate when change is prevented" test, but controlled):

| variant | `validate` calls | server error |
| --- | --- | --- |
| base | 0 | still shown |
| head | 1 | cleared |

Probe H4 (controlled, handler calls `details.cancel()` and still calls `setValue`): head validates once and sets dirty, the same as base, so `cancel()` has no effect at all in controlled mode.

Both of these may well be the desired behaviour — if the prop changed, the value changed. The point is that the existing prevented-event test only covers the uncontrolled branch, so the suite stays green while the controlled half of that contract silently flips, and the PR body's sentence "`details.cancel()` in `onValueChange` now stops the internal handling" is true for half the handling in one of the two modes.

### Worked proposal

Name the sync once, and let each trigger decide only *whether* to run it:

```tsx
const syncFieldState = useStableCallback((nextValue: string) => {
  clearErrors(name);
  // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
  setDirty(nextValue !== (validityData.initialValue ?? ''));
  setFilled(nextValue !== '');
  validation.change(nextValue);
});

useValueChanged(serializedValue, () => {
  if (serializedValue !== undefined) {
    syncFieldState(serializedValue);
  }
});
```

```tsx
onChange(event) {
  const inputValue = event.currentTarget.value;
  const details = createChangeEventDetails(REASONS.none, event.nativeEvent);
  onValueChange?.(inputValue, details);

  // Controlled values sync from the `value` prop instead, so that a value the consumer
  // rejects or rewrites never reaches the field state.
  // `defaultPrevented` is a workaround for https://github.com/react/react/issues/9023
  if (isControlled || details.isCanceled || event.nativeEvent.defaultPrevented) {
    return;
  }

  syncFieldState(inputValue);
},
```

That removes the duplicated block, the nested condition, and the order difference. The handler becomes "notify, decide, sync", and the half-applied state can no longer be expressed.

This was built and run (`scratch/JudoFieldControl.tsx`). `scratch/ported.test.tsx` re-states nine of the PR's assertions with plain Testing Library (the two render-count tests, validate-once in both modes, numeric dirty reset, programmatic sync, filled-on-mount in both modes, cancel does not validate, prevented change does not clear errors or validate, required error on clearing a prefilled value) and runs them against head and judo: 18 of 18 pass.

It is not strictly behaviour-preserving, and that is deliberate. With the judo variant probe H2 reports `filled=yes dirty=yes invalid=yes error=bad` after the cancelled clear — all three now describe the same previous value. Probe H7 is unchanged (`validate` still receives `a`), because skipping the whole sync still skips the timer reset. If that stale debounce is not acceptable, the second option above is the one to take: drop `details.isCanceled` from the guard, delete the "does not validate when the change is canceled" test, and document that an uncontrolled native input cannot be cancelled. Either way the decision becomes one line in one place.

## Finding D — the eleventh file to hand-write the same sequence

The PR body's argument is that `Field.Control` was the last control not syncing through `useValueChanged`. That is accurate, and it is also the argument for not writing the sequence out again. Non-test call sites of `validation.change(` in `packages/react/src`:

```
checkbox/root/CheckboxRoot.tsx          checkbox-group/CheckboxGroup.tsx
radio-group/RadioGroup.tsx              switch/root/SwitchRoot.tsx
slider/root/SliderRoot.tsx              select/root/SelectRoot.tsx
combobox/root/AriaCombobox.tsx (x2)     otp-field/root/OTPFieldRoot.tsx
number-field/input/NumberFieldInput.tsx number-field/root/NumberFieldRoot.tsx
field/control/FieldControl.tsx (x2)
```

`grep -rn "clearErrors(name)"` over non-test source returns 12 lines. Each site pulls `clearErrors` from the form context, `setDirty` / `setFilled` / `validityData` / `validation` from the field context, and re-assembles `clearErrors → setDirty → [setFilled] → validation.change` by hand.

The copies have already drifted. `SliderRoot.tsx:181` calls `validation.change(fieldValue)` and only then `setDirty(isDirty)` at line 190, which is the opposite of the order `FieldControl.tsx:148` says is required. Whether that is a live bug in Slider is outside this diff; it is the evidence that a convention kept in ten files by repetition does not stay a convention.

The concept already has an owner. `FieldRoot` owns `markedDirtyRef`, `setDirty`, `setFilled` and `validation`, and already reads the form context. A single action on the field context would make the ordering constraint a property of one function:

```tsx
// FieldRoot.tsx, exposed on FieldRootContext
const syncValue = useStableCallback((value: unknown, next: { dirty: boolean; filled?: boolean }) => {
  clearErrors(effectiveName);
  // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
  setDirty(next.dirty);
  if (next.filled !== undefined) {
    setFilled(next.filled);
  }
  validation.change(value);
});
```

```tsx
// FieldControl.tsx — replaces `syncFieldState` above, and the `useFormContext()` call
syncValue(nextValue, {
  dirty: nextValue !== (validityData.initialValue ?? ''),
  filled: nextValue !== '',
});
```

```tsx
// SwitchRoot.tsx
useValueChanged(checked, () => {
  syncValue(checked, { dirty: checked !== validityData.initialValue, filled: checked });
});
```

Each control keeps what is specific to it (how "dirty" is computed, the OTP focus hand-off, the radio fallback input) and loses the part that is identical everywhere. This sketch was not built or run. Migrating all ten siblings does not belong in this PR; introducing the action here, where the newest copy is being written, and converting the siblings afterwards does.

## Considered and not raised

- **Registering `undefined` instead of `serializedValue`.** `getValueFromInput` is always supplied as the form-value override, so the registration's `value` is only consumed once, by `captureInitialValue`. Passing `undefined` would take the baseline from the DOM and stop the registration effect re-running on every controlled keystroke. It was rejected: the controlled dirty check compares `String(prop)`, and a DOM-read baseline can differ from that wherever the element sanitises its value, so prop-against-prop is the safer pairing.
- **`String(value)` as the serialisation.** It matches how React writes a non-string `value` to an `<input>`; probe H9 confirms a numeric `value={5}` now returns to pristine at head where base stayed dirty. It would not match a `render`-ed `<select multiple>` with an array value. That is an exotic use of `Field.Control` and was not tested.
- **The `serializedValue === undefined` early return.** It only triggers when a controlled value becomes `null` or the control flips to uncontrolled, both of which React already warns about.
- **File size.** 207 lines for the component, 273 for the test file.
