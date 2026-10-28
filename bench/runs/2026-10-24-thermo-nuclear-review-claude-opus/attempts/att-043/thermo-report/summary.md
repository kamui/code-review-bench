# Thermo Nuclear Review — mui/base-ui#5460 "[field] Sync controlled value changes with field state"

Range reviewed: `30b8ea2004fa999bed151204208676c6c0a9d261..14d39e5d1ad6b7aca2fb067415dba09c6bea219b` (`git diff main...review-head`), 2 files: `packages/react/src/field/control/FieldControl.tsx` and its test file.

## What the change does

In controlled mode, `Field.Control` now skips its own field-state handling in `onChange` (`FieldControl.tsx:142-146`). It drives dirty, filled, clearErrors, and validation from a new `useValueChanged(serializedValue, …)` layout effect (`FieldControl.tsx:105-115`). It registers `String(value)` with the field instead of the raw prop (`FieldControl.tsx:85-97`). It replaces the old value-tracking filled effect with a mount-only DOM check (`FieldControl.tsx:99-103`). In the uncontrolled path, `details.isCanceled` now gates `clearErrors` and validation (`FieldControl.tsx:153`).

How I checked it:

- Read the change against `useFieldValidation` (`change`/`commit`), `FieldRoot` (`setDirty`/`markedDirtyRef`), `useFieldControlRegistration` (`captureInitialValue`/`validate`), `useValueChanged`, and the sibling controls (`SwitchRoot`, `NumberFieldInput`).
- Ran the existing jsdom suites for field, fieldset, input, and form: 12 files, 303 passed, 9 skipped.
- Wrote scratch tests in the work directory and ran each one against both the head tree and an exported copy of the base tree (`git archive main`). That shows which differences this PR actually introduces.

No security, devex, or feature-gate problems: the change is local to one client component and touches no configuration, scripts, environment, or flags.

## Findings

### 1. [Medium] Resetting a required controlled field to empty now shows a "Required" error right away, while the field says it is not dirty

- **Where:** `packages/react/src/field/control/FieldControl.tsx:105-115`
- **What happens:** a programmatic change now runs `validation.change(serializedValue)`.
  - In `validationMode="onChange"`, or `"onSubmit"` after a submit attempt, `shouldValidateOnChange()` is true. So `useFieldValidation.change` (`field/root/useFieldValidation.ts:316-328`) calls a full `commit(value)`, not a revalidate.
  - Inside `commit`, `getState` hides `valueMissing` only while `markedDirtyRef.current` is false (`useFieldValidation.ts:223-228`). But `FieldRoot.setDirty` only ever sets `markedDirtyRef` to true and never back (`field/root/FieldRoot.tsx:69-78`).
  - The new effect calls `setDirty(false)` first, because the value equals the initial `''`. That clears `data-dirty`, but `markedDirtyRef` stays true from the user's earlier typing.
  - So the commit publishes `valueMissing` and `Field.Error match="valueMissing"` renders.
- **Evidence:** scratch test. Controlled `required` input with `validationMode="onChange"`: type `"abc"`, then a reset button calls `setValue('')`.
  - Base: "Required" is not shown.
  - Head: "Required" is shown, and `data-dirty` is absent.
- **Impact:** the PR body names "a form library reset" as the motivating case. After this change, resetting a form where the user had typed into required fields marks every one of them invalid (`aria-invalid="true"`, error text visible). At the same time the field reports itself pristine. That state contradicts itself, and it's a visible UX regression in the scenario the PR targets. `SwitchRoot` has the same pattern, but it is new behavior for `Field.Control`/`Input`, and no test covers a reset to the initial value.

### 2. [Low] A controlled value filled in after mount (async-loaded edit forms) now marks the field dirty and runs validation

- **Where:** `packages/react/src/field/control/FieldControl.tsx:105-115`, together with `captureInitialValue` in `internals/field-register-control/useFieldControlRegistration.ts`
- **What happens:**
  - The baseline (`initialValue`) is captured once, from the first registration (`''`), and is deliberately never recaptured.
  - When the app loads data and sets `value` from `''` to the loaded text, `useValueChanged` runs `setDirty('loaded' !== '')`. That sets `data-dirty`, sets `markedDirtyRef.current = true`, and in onChange mode calls the user's `validate`.
- **Evidence:** scratch test. Controlled field starting at `''`, then `setValue('loaded')` in a `setTimeout` from `useEffect`.
  - Base: `data-dirty` absent, `validate` called 0 times.
  - Head: `data-dirty=""`, `validate` called once.
- **Impact:**
  - Common "fetch, then populate" edit forms now show every field as dirty before the user touches anything.
  - Validators run against server data.
  - Because `markedDirtyRef` is now latched, `valueMissing` noise suppression is turned off for those fields.
  - Consumers have to remount or key `Field.Root` to get a clean baseline. Neither the PR nor the docs mention this.

### 3. [Low] Remounting a controlled `Field.Control` with an empty value leaves `data-filled` set from the previous instance

- **Where:** `packages/react/src/field/control/FieldControl.tsx:99-103` (replaces the removed effect that called `setFilled(false)` when `valueProp === ''`)
- **What happens:**
  - The new mount effect only ever sets filled to true. `useValueChanged` doesn't fire on mount, because its ref starts at the mount value.
  - The removed effect ran on mount and called `setFilled(false)` for an empty controlled value.
  - `filled` lives in `Field.Root` state. So when a control unmounts and remounts inside the same root (conditional rendering, step-by-step forms), the old filled state survives.
- **Evidence:** scratch test. Controlled control with value `'abc'` is unmounted, the value is set to `''`, and the control is remounted.
  - Base: `data-filled` absent.
  - Head: `data-filled=""` on an empty input.
- **Impact:** wrong `data-filled` styling on an empty field, for example floating labels drawn as if text were present.

### 4. [Low] Non-string controlled values now reach `validate` and `Field.Validity` as strings at submit and blur-commit time

- **Where:** `packages/react/src/field/control/FieldControl.tsx:85-97`
- **What happens:**
  - The registration value changes from the raw prop to `String(value)`.
  - `useFieldControlRegistration`'s `validate` (used by Form submit) commits `registration.value` when it is defined. `captureInitialValue` stores it as `initialValue`. Both end up in `validityData`, which `Field.Validity` exposes.
- **Evidence:** scratch test. `<Form><Field.Root name="n" validate={fn}><Field.Control value={5} …/></Field.Root></Form>`, then submit.
  - Base: `fn` receives `5`.
  - Head: `fn` receives `"5"`.
- **Impact:**
  - Validators that check `typeof value === 'number'` or `Array.isArray(value)` stop matching at submit. Array values become `"a,b"`.
  - `validity.value` and `validity.initialValue` change type for `Field.Validity` render consumers.
  - The change makes the dirty comparison correct, and the change-path value was already a string. But the PR presents it as internal, and it is a public behavior change at the submit boundary, with no test.

### 5. [Low] The PR description promises fixes and design details the commit doesn't contain

- **Where:** PR body vs `packages/react/src/field/control/FieldControl.tsx` (whole diff)
- **What's missing or different:**
  - The body says "A control that unmounts or becomes disabled while focused no longer latches `data-focused`". The diff has no unmount or disabled handling for focus: the `autoFocus` effect is only moved (`FieldControl.tsx:117-123`), and `onBlur` is unchanged. The diff also adds no focus test.
  - The body describes a `processedValueRef` guard. It doesn't exist. Controlled mode instead returns early from `onChange`.
  - The body's "Open question" says the PR does not call `clearErrors(name)` on value changes. The code does call it (`FieldControl.tsx:110`), so a programmatic change now clears Form server errors for `Field.Control`.
- **Impact:**
  - The focus-latch bug the PR claims to fix is still present.
  - Reviewers and release notes would describe behavior that doesn't ship. That includes the server-error clearing the author asked reviewers to decide on.

## Checked and not reported

- **Double validation per controlled keystroke:** none. `onChange` returns early in controlled mode, and the new test covers this.
- **Stale async validators after a programmatic change:** they are discarded by the `validationCommitIdRef` bump.
- **Controlled `preventDefault`ed input events:** base and head behave the same (verified).
- **`value={null}` typing:** no longer updates filled or dirty, but the prop type excludes `null`.
