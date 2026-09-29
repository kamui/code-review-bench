# Detail: Field.Control (`packages/react/src/field/control/`)

## Scope and commands

- `git diff main...review-head` for both changed files; `FieldControl.tsx` is 207 lines after the change, the test file 273.
- Existing suite: `TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx` gives 26 passed, 1 skipped.
- Sibling comparison: `useValueChanged` is used in Checkbox, CheckboxGroup, RadioGroup, Switch, Slider, NumberFieldInput, Form and `useOpenInteractionType`. `NumberFieldInput.tsx:101` is the closest analogue and does not itself gate on `defaultPrevented`.
- Scratch probes (outside the clone, `@testing-library/react`, aliases to `packages/react/src`), each run once:
  1. Controlled `Field.Control` (`value` + `onValueChange={setValue}`), `validationMode="onChange"`, a capture-phase `input` listener calling `preventDefault()`. Result: `validate` called 1 time. Verification: CONFIRMED.
  2. Uncontrolled `Field.Control` with `onValueChange={(v, d) => d.cancel()}`, then a change to `'a'`. Result: `data-dirty` present and `data-filled` present. Verification: CONFIRMED.
- Not verified (browsers unavailable): real-browser behavior of React issue 9023, and the reported render counts (I read the code but did not re-measure).

## F1 detail: prevented change in controlled mode

Base handler (before): one `onChange` doing `setDirty`, `setFilled`, then `if (!defaultPrevented) { clearErrors; validation.change }`.
After: uncontrolled keeps that shape; controlled returns early and relies on the effect, which has no prevented signal. React fires `onChange` from the `input` event even when default is prevented, which is why the workaround exists. In controlled mode the consumer's `setValue` still changes the prop, so the effect fires.

Worked proposal: record `event.nativeEvent.defaultPrevented || details.isCanceled` into a `skipNextSyncRef` inside `onChange`; the controlled effect reads and clears it. If the maintainers prefer "the consumer owns controlled semantics", delete the workaround comment from the uncontrolled branch and document the asymmetry instead. Either way, add a controlled twin of the existing prevented test.

## F2 detail: partial cancel

`onChange` order is: call `onValueChange` -> (controlled? return) -> `setDirty` -> `setFilled` -> `if (!prevented && !canceled) { clearErrors; validation.change }`. Cancel affects only the last step. The code comment above `setDirty` justifies the ordering by `markedDirtyRef`, which shows dirty is coupled to validation. That coupling makes half-application worse: `markedDirtyRef` becomes true even though validation was skipped, so a later `valueMissing` suppression (`useFieldValidation.ts:225`) behaves as if the user had changed the field.

Worked proposal (single helper, see F3):

```ts
const syncFieldState = useStableCallback((next: string) => {
  // dirty first: validation.change reads markedDirtyRef
  setDirty(next !== (validityData.initialValue ?? ''));
  setFilled(next !== '');
  clearErrors(name);
  validation.change(next);
});
// controlled:   useValueChanged(serializedValue, () => serializedValue !== undefined && syncFieldState(serializedValue));
// uncontrolled: onChange: if (!event.nativeEvent.defaultPrevented && !details.isCanceled) syncFieldState(inputValue);
```

This deletes the duplicated block and the ordering comment, and makes cancel all-or-nothing. If the uncontrolled DOM must still show dirty/filled after a cancel, split `syncFieldState` into `syncFlags` (always) and `validate` (gated) so that intent is explicit.

## F3 detail: duplicated sequence

Copy A: `FieldControl.tsx:110-114`. Copy B: `FieldControl.tsx:148-156`. Differences: order of `clearErrors` vs `setDirty`, the guards, and the value source. Ordering differs between copies (A calls `clearErrors` first, B calls it last), which is unintentional drift and not a design choice.

## F4 detail: filled paths

`filled` is written in three places: mount effect (only sets true), controlled effect (sets both ways), uncontrolled `onChange` (sets both ways). The mount effect runs regardless of mode. With the helper from F3, the mount effect could become `if (!isControlled && inputRef.value) setFilled(true)`, or the controlled case could rely on the same effect via a one-time initial call. Behaviorally equivalent, one fewer reasoning path.

## F5 detail: serialization boundary

`serializedValue` is used for three purposes at once: registration value, effect dependency, dirty comparison. `null` and `undefined` collapse together; `isControlled` is computed separately from `valueProp !== undefined`, so `value={null}` is "controlled" for the input (`value: null`, which React warns about) but "no value" for sync. Suggest one derived object or a typed helper so the three meanings stay in agreement.

## F6 detail: churn

Diff moves the `inputRef` + autoFocus effect below `useValueChanged` and rewrites the mount effect in place. No behavior change results from the move.

## F7 detail: tests

- Present and good: numeric dirty round trip, programmatic controlled change, empty controlled mount, cancel-not-validated, validate-once controlled.
- Missing: `onChange`-mode render count (PR reports 2 per keystroke), controlled prevented event (F1), cancel then dirty/filled (F2), consumer rewrites value (for example uppercases it) so the field state must only ever see the rewritten value, async validator race on programmatic reset, programmatic reset clearing a visible `Field.Error`.
- The "renders once per keystroke" name is slightly misleading: it asserts `settled + 2` for two keystrokes in `onSubmit` mode only.
