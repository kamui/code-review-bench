# Thermo nuclear review — mui/base-ui#5460 "[field] Sync controlled value changes with field state"

Range: `30b8ea200..14d39e5d1`, two files (`FieldControl.tsx`, `FieldControl.test.tsx`). I read the diff, `useRegisterFieldControl`, `useFieldControlRegistration`, `useFieldValidation.change/commit`, `FieldRoot.setDirty` and `useValueChanged`. I also ran scratch jsdom tests against the head, outside the clone. There are no security or devex issues. The findings below are behavioral.

## Finding 1 — Medium: controlled `Field.Control` now validates and clears errors for a prevented change event

**File:** `packages/react/src/field/control/FieldControl.tsx`, lines 105–114 (new `useValueChanged` block) and 137–157 (`onChange`).

**Evidence:**
- The `onChange` handler returns early when `isControlled` (line 144). The `event.nativeEvent.defaultPrevented` guard (line 153, the React #9023 workaround) now only covers the uncontrolled branch.
- A controlled field instead validates from the `useValueChanged` effect, which calls `clearErrors(name)`, `setDirty`, `setFilled` and `validation.change(serializedValue)` unconditionally.
- The existing test "does not clear errors or validate when change is prevented" is uncontrolled only, so it does not cover this.
- Scratch test: a controlled `<Field.Control value={v} onValueChange={setV}/>` inside `<Form errors={{message:'Server error'}}>` with `validationMode="onChange"` and a `validate` spy. A capturing `input` listener calls `preventDefault()`, then `fireEvent.input(..., {cancelable: true})`. `validate` was called once. The same scenario uncontrolled gives zero calls (that is the existing test).

**Failure:** Consumers or libraries that call `preventDefault()` on the input event to opt out of Field's validation and error clearing now get the opposite on controlled inputs. Validation runs and `clearErrors` fires for a change the existing contract says must be ignored. This silently changes behavior for controlled users, who are the likeliest to use the guard.

## Finding 2 — Medium/Low: `details.cancel()` is only honored on the uncontrolled path, and is only partial there

**File:** `packages/react/src/field/control/FieldControl.tsx`, lines 137–157 (`onChange`) and 105–114.

**Evidence:**
- `details.isCanceled` is checked only at line 153, after the `if (isControlled) return`. When controlled, `cancel()` has no effect on the echo-driven effect.
- Scratch test: a controlled control with `onValueChange={(x, d) => { d.cancel(); setV(x); }}` and `validationMode="onChange"`. `validate` was called once.
- In the uncontrolled branch, `setDirty` and `setFilled` (lines 149–150) run before the cancel check. A canceled change still sets `data-dirty` and `data-filled` (scratch test: both `true`), and only validation and `clearErrors` are skipped.

**Failure:** The PR says `details.cancel()` now "stops the internal handling". For controlled use it does not stop anything. For uncontrolled use dirty and filled still update, so the Field state and the validation state diverge. The new test "does not validate when the change is canceled" covers only validation in the uncontrolled case. If cancel is meant to apply when controlled, it needs to be remembered and consulted in the effect. If it is meant to apply only to uncontrolled use, that should be documented. Dirty and filled should also be gated consistently.

## Finding 3 — Low: the code does not match the PR description (claimed guard and focus fix absent, `clearErrors` behavior inverted)

**File:** `packages/react/src/field/control/FieldControl.tsx`, lines 105–114 (`clearErrors(name)` at 110) and 119–123.

**Evidence:**
- The PR body says `processedValueRef` prevents double handling. It does not exist in the diff. Double validation is avoided only because the controlled `onChange` skips everything and defers to the effect.
- The PR body says a control that unmounts or becomes disabled while focused no longer latches `data-focused`. The diff has no such change. Only the `autoFocus` effect was moved.
- The PR body's open question says this PR does not call `clearErrors` on programmatic changes. The code does (line 110). Any controlled value change, including a form-library reset or an app normalizing a value after a server response, now clears the matching `<Form errors>` entry.

**Failure:** A reviewer or maintainer relying on the description will have the wrong picture of what shipped. The `data-focused` latch bug is not fixed. Server errors are now cleared by programmatic value changes, which is a user-visible behavior change that is the opposite of what was described. Either the description or the code should be corrected.

## Notes (not findings)

- `validate` and `validation.change` now receive the serialized string for programmatic changes (for example `"5"` for `value={5}`), consistent with typed input.
- The dirty baseline now comes from the serialized value, so numeric and array controlled values compare correctly. The new tests pass.
