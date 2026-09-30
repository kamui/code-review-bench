# 02 — The field value-sync sequence across controls (canonical layer)

Scope: how the new `useValueChanged` block in `packages/react/src/field/control/FieldControl.tsx`
(lines 105–115) relates to the same block in the other Field-aware controls.

## Evidence

Search: `useValueChanged\(` over `packages/react/src`, non-test files. Every Field-aware control
now carries a hand-written copy of the same sequence — clear form errors for the field name, recompute
dirty against `validityData.initialValue`, recompute filled, call `validation.change(value)`:

| File | Lines | Dirty comparison | Filled |
| --- | --- | --- | --- |
| `switch/root/SwitchRoot.tsx` | 105–111 | `checked !== initialValue` | `checked` |
| `checkbox/root/CheckboxRoot.tsx` | 210–220 | `checked !== initialValue` | `checked` |
| `checkbox-group/CheckboxGroup.tsx` | 119–132 | `!areArraysEqual(value, initial)` | `value.length > 0` |
| `radio-group/RadioGroup.tsx` | 176–189 | `checkedValue !== initialValue` | `checkedValue != null` |
| `select/root/SelectRoot.tsx` | 243–248 | `isSelectedValueDirty(…)` | (elsewhere) |
| `slider/root/SliderRoot.tsx` | 178–191 | arrays via `areArraysEqual`, else `!==` | (elsewhere) |
| `otp-field/root/OTPFieldRoot.tsx` | 199–203 | `value !== initialValue` | (elsewhere) |
| `combobox/root/AriaCombobox.tsx` | 1055–1079 (two copies) | `isSelectedValueDirty(…)` / `!==` | (elsewhere) |
| `number-field/input/NumberFieldInput.tsx` | 101–110 | (root) | (root) |
| **`field/control/FieldControl.tsx` (this PR)** | **105–115** | `String(value) !== (initialValue ?? '')` | `!== ''` |

The copies already disagree on ordering (Slider validates before setting dirty; Switch, RadioGroup,
FieldControl set dirty first; Checkbox sets filled first), on whether `clearErrors` is guarded by a
defined name (CheckboxGroup guards; the rest do not), and on whether `setFilled` lives in the block.
`validation.change` depends on `markedDirtyRef`, which `setDirty(true)` sets, so ordering is not
cosmetic: a copy that validates before marking dirty sees the previous dirty flag.

## Finding 4 — The PR adds a tenth copy instead of giving the sequence a canonical home

This PR is exactly the moment the pattern became universal: with `Field.Control` joining, every
Field-aware control syncs through the same four steps. Rather than inlining another copy, the
sequence belongs in the field layer that already owns `setDirty`, `setFilled`, `validation` and the
`markedDirtyRef` ordering rule. A worked shape, living next to `useRegisterFieldControl` in
`internals/field-register-control/` (or exposed from `FieldRootContext`):

```ts
export function useFieldValueSync<Value>(
  value: Value,
  options: {
    name: string | undefined;
    isDirty: (value: Value, initialValue: unknown) => boolean;
    isFilled?: ((value: Value) => boolean) | undefined;
    enabled?: boolean | undefined;
  },
) {
  const { setDirty, setFilled, validation, validityData } = useFieldRootContext();
  const { clearErrors } = useFormContext();

  const sync = useStableCallback((nextValue: Value) => {
    // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
    setDirty(options.isDirty(nextValue, validityData.initialValue));
    if (options.isFilled) {
      setFilled(options.isFilled(nextValue));
    }
    clearErrors(options.name);
    validation.change(nextValue);
  });

  useValueChanged(value, () => {
    if (options.enabled !== false) {
      sync(value);
    }
  });

  return sync; // for imperative triggers such as Field.Control's uncontrolled onChange
}
```

`Field.Control` would then read:

```ts
const syncFieldValue = useFieldValueSync(domValue, {
  name,
  enabled: domValue !== undefined,
  isDirty: (next, initial) => next !== (initial ?? ''),
  isFilled: (next) => next !== '',
});
```

and its uncontrolled `onChange` would call the returned `syncFieldValue(inputValue)`, which also
resolves Finding 1 of `01_field-control-sync.md`.

Severity and sequencing: this is a follow-up, not a blocker for this PR. The PR should not be asked to
migrate nine other controls. But it should not be the change that makes the pattern ten-wide without
at least landing the helper for `Field.Control` itself (so the next migration has a target), or
filing the consolidation explicitly. Controls with extra steps (RadioGroup's fallback input re-point,
OTP Field's pending focus, Combobox's label sync, Number Field's `blockRevalidationRef`) keep those
steps in their own `useValueChanged` or in a follow-on call after `sync`.

## Verification status

Pattern inventory: verified by search and reading each site at `review-head`. The helper is a design
proposal; it was not implemented or run (the clone is read-only for this review).
