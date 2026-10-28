# Thermo-nuclear review: mui/base-ui#5460 `[field] Sync controlled value changes with field state`

Range `30b8ea2..14d39e5`, two files: `FieldControl.tsx` and `FieldControl.test.tsx`. I ran the existing field test suite (240 passed) and wrote scratch jsdom tests outside the clone to check the findings below. There are no security issues. The three findings below are correctness regressions or gaps, none of them high priority.

## Finding 1 (Medium): controlled `Field.Control` no longer honours the React #9023 `defaultPrevented` guard

**File:** `packages/react/src/field/control/FieldControl.tsx`, lines 137-157 (the `onChange` early return at 144-146 and the unguarded sync at 105-115).

**Evidence:**
- On `main`, `onChange` ran `clearErrors(name)` and `validation.change(inputValue)` only when `!event.nativeEvent.defaultPrevented`. This applied to controlled and uncontrolled inputs. The existing test "does not clear errors or validate when change is prevented" encodes that contract, but it only covers the uncontrolled case.
- In this PR, controlled inputs return before that check (lines 144-146). Their validation, `clearErrors` and dirty/filled sync now run from `useValueChanged` (lines 105-115), which never looks at `defaultPrevented` or `details.isCanceled`.
- Scratch test: a controlled `<Field.Control value onValueChange={setV}>` in `validationMode="onChange"` with a `validate` spy. A capture-phase `input` listener calls `preventDefault()` on the native event, then `fireEvent.input(... 'a')`. `validate` was called once. On `main` the same scenario skipped validation.
- The same applies to `details.cancel()`. With a controlled consumer that calls `details.cancel()` and then `setV(x)`, `validate` still ran once. The PR text says "`details.cancel()` in `onValueChange` now stops the internal handling", but that is true only for uncontrolled inputs. The new "does not validate when the change is canceled" test is uncontrolled only.

**Failure:** Controlled consumers that suppress or veto a change via `preventDefault` or `cancel`, but still update their state, now get validation, error clearing and dirty/filled updates for it. The `defaultPrevented` guard and the cancel fix were both meant to prevent exactly that. The guard is a behaviour change for the most common way of using `Field.Control` (controlled), and no test covers it.

## Finding 2 (Low): the controlled echo adds a second synchronous render per keystroke when validation runs on change

**File:** `packages/react/src/field/control/FieldControl.tsx`, lines 105-115. The related test is at `FieldControl.test.tsx` lines 47-80.

**Evidence:**
- The controlled path now calls `validation.change` from a layout effect that runs after the render triggered by `setValue`.
- In `validationMode="onChange"` (or after a submit attempt), `commit` always ends in `setValidityData(nextValidityData)` with a fresh object. This is a state update inside a layout effect, so React re-renders synchronously a second time.
- Scratch test, controlled `validationMode="onChange"` with a counting render prop: 2 renders per keystroke. `main` batched the handler's state updates into one.
- The added test "renders once per keystroke for controlled input changes" only exercises the default `onSubmit` mode. There `commit(value, true)` returns early because the field is valid, so the test passes and its comment ("must not schedule a second render per keystroke") does not hold in on-change mode.

**Failure:** Controlled fields that validate on change re-render the control and every Field consumer twice per keystroke, which is a performance regression. It matters more with a heavy `render` prop or `Field.Validity`, and it is not covered by the new test.

## Finding 3 (Low): described unmount/disabled focus fix is absent from the change

**File:** `packages/react/src/field/control/FieldControl.tsx`, lines 119-123 and 158-168. The only focus-related code is unchanged: `setFocused(true)` in `onFocus`, `setFocused(false)` in `onBlur`, and the `autoFocus` layout effect.

**Evidence:**
- The PR body says "A control that unmounts or becomes disabled while focused no longer latches `data-focused`", and that `processedValueRef` guards against double validation.
- The diff contains neither a `processedValueRef` nor any cleanup or disabled handling for `focused`. A disabled or unmounted focused input fires no blur, so `setFocused(false)` is never called.
- The echo guard is also absent. In practice, a controlled keystroke is validated once because the `onChange` path now returns early for controlled inputs, so the described double-validation risk does not arise. Only the focus fix is truly missing.

**Failure:** A focused `Field.Control` that becomes `disabled` or unmounts leaves `data-focused` set on the Field. The merged change does not fix the bug it says it fixes.

## Questions

- The PR body says it does not call `clearErrors(name)` on programmatic value changes. The code does call it, at line 110, inside the `useValueChanged` callback, so a programmatic change does clear Form server errors on `Field.Control`. Is that intended? If so, the "Open question" in the description is already settled in code.
