# 01: Field.Control implementation (`packages/react/src/field/control/FieldControl.tsx`)

Commands used: `git diff main...review-head -- packages/react/src/field/control/FieldControl.tsx`; reading `internals/useValueChanged.ts`, `internals/field-register-control/useRegisterFieldControl.ts`, `useFieldControlRegistration.ts`, `field/root/FieldRoot.tsx`, and the sibling uses in `SwitchRoot.tsx` and `NumberFieldInput.tsx`. Existing suite: `TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field` gave 240 passed, 9 skipped.

Scratch tests were run from the work directory with a link to the clone's `node_modules` and alias config. The clone was not modified.

## 1. Two copies of the sync sequence (verified by reading)

Controlled copy, lines 105-115: `clearErrors(name)`, `setDirty(serialized !== (initialValue ?? ''))`, `setFilled(serialized !== '')`, `validation.change(serialized)`.
Uncontrolled copy, lines 137-157: `onValueChange`, early return if controlled, `setDirty`, `setFilled`, then if not `defaultPrevented` and not canceled, `clearErrors(name)` and `validation.change(inputValue)`.

Differences: order of `clearErrors` relative to `setDirty`; the ordering comment about `markedDirtyRef` exists only on the uncontrolled copy (the controlled copy is also correct because `setDirty(true)` writes `markedDirtyRef` synchronously, but a reader has to know that); gating on `defaultPrevented`/`isCanceled` exists only on one side.

Worked code-judo sketch (behavior preserving for uncontrolled mode):

```ts
const syncFieldState = useStableCallback((nextValue: string, options?: { validate?: boolean }) => {
  // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
  setDirty(nextValue !== (validityData.initialValue ?? ''));
  setFilled(nextValue !== '');
  if (options?.validate !== false) {
    clearErrors(name);
    validation.change(nextValue);
  }
});

useValueChanged(serializedValue, () => {
  if (serializedValue !== undefined) syncFieldState(serializedValue);
});
// onChange: if (!isControlled) syncFieldState(inputValue, { validate: !defaultPrevented && !details.isCanceled });
```

This removes the duplicated block and puts the guards next to the one call that needs them. If the contract from sections 2 and 3 is decided as "prevented or canceled changes never reach field state", the `validate` option disappears and the guard becomes an early return.

## 2. Lost `defaultPrevented` guard for controlled inputs (verified by scratch test)

Scratch test: controlled `Field.Control` (`value` state, `onValueChange={setV}`) in `<Form errors={{ message: 'Server error' }}>` with `validationMode="onChange"` and a capture-phase `input` listener that calls `preventDefault()`, followed by `fireEvent.input(control, { cancelable: true, target: { value: 'a' } })`.
Result: `validate` called 1 time. The uncontrolled version of the same scenario is the existing test and expects 0 calls.
The pre-PR `onChange` applied the `defaultPrevented` gate to both modes, so this is a controlled-mode behavior change that is not mentioned in the PR body.

## 3. Cancel semantics (verified by scratch test)

Scratch test: controlled control with `onValueChange={(n, d) => { setV(n); d.cancel(); }}`, `validationMode="onChange"`. Result: `validate` called 1 time, `data-dirty` present.
In uncontrolled mode, lines 149-150 run `setDirty` and `setFilled` before the `isCanceled` check at line 153, so a canceled change still flips dirty and filled. The PR text "stops the internal handling" is therefore only partly true in uncontrolled mode and not true in controlled mode when the consumer applies the value.

Options: (a) declare cancel as "do not sync anything" and move the check to the top of `onChange`, with a ref consulted by the controlled effect; (b) declare cancel as "skip validation only" and document it. Option (a) is simpler.

## 4. Mount-time filled effect (lines 99-103)

It reads the DOM value for both modes. It is the only source of initial `filled` for a controlled control with a non-empty `value` (the controlled effect does not fire on mount because `useValueChanged` compares with the mount value). Comment the intent or co-locate it with registration. Also note: the effect no longer clears filled for `value=""` on mount, which is fine because the default is unfilled, and the new test "does not set filled state on mount for an empty controlled value" pins that.

## 5. Value typing (line 84-86)

`valueProp` is `string | number | readonly string[] | undefined` (plus `null` at runtime); `value == null ? undefined : String(value)` is the bridge. `isControlled = valueProp !== undefined` means `value={null}` is controlled but produces `serializedValue === undefined`, which the effect treats as skip. The effect's `undefined` check conflates uncontrolled with null. Prefer normalizing once and documenting.

## Verification status

Sections 2 and 3: confirmed by scratch execution. Sections 1, 4 and 5: confirmed by reading. Chromium-only behavior (real browser events) was not exercised.
