# Thermo-nuclear code quality review: mui/base-ui#5460 (Field.Control controlled sync)

Range: `30b8ea2004fa..14d39e5d1ad6` (`git diff main...review-head`), 2 files: `FieldControl.tsx` (207 lines after the change, well under the 1k threshold) and `FieldControl.test.tsx`. The existing FieldControl test file passes offline in jsdom (26 passed, 1 skipped). Detail evidence and worked proposals are in `01_field-control.md`.

## Verdict

The direction is right. Moving the controlled path onto `useValueChanged`, the same primitive the sibling controls use, is the correct ownership boundary, and it deletes the old mount/valueProp filled effect. It also fixes the `"5" !== 5` dirty-baseline bug at its root by registering the serialized value. I would not call this a structural regression. It falls short of the approval bar on one point. The change leaves `FieldControl` with two hand-written copies of the same "apply a new string value to field state" sequence, and the two copies have already drifted apart on cancel and `preventDefault` handling. The findings below are ordered by importance. None is a file-size problem.

## Findings

### 1. The "apply value to field state" sequence is written twice with different guards

`FieldControl.tsx` lines 105-115 (the `useValueChanged` callback) and lines 143-157 (the `onChange` handler) each perform the same four steps: compare against `validityData.initialValue ?? ''` and call `setDirty`, call `setFilled(value !== '')`, call `clearErrors(name)`, and call `validation.change(value)`. The PR description calls this "one owner per mode", but what actually exists is two owners that each carry a full copy of the pipeline. The copies differ only in guards, and the guards are where the trouble starts (finding 2). A reader has to diff the two blocks by eye to see what is intentionally different. The remedy is a single local `syncValue(next: string, { validate }: ...)` helper, or a small `useFieldValueSync` hook shared with the other Field-backed controls. It would own the dirty comparison, the filled flag, error clearing and the `validation.change` call, in the order that `markedDirtyRef` requires. Each mode would then call it with only the guard that is specific to it. The ordering constraint "update dirty before validating" currently lives in a comment at one call site and is silently relied on at the other. The helper would enforce it in one place.

### 2. Cancel and preventDefault handling is inconsistent, and the new cancel guard is only half applied

In the uncontrolled `onChange` path (lines 148-157), `setDirty` and `setFilled` run unconditionally, and only `clearErrors` and `validation.change` sit behind `!defaultPrevented && !details.isCanceled`. The PR body says `details.cancel()` "now stops the internal handling", but a canceled change still flips `data-dirty` and `data-filled` while the input keeps the typed text, so the field state and the validation state disagree. The controlled path is different again. It returns before the `defaultPrevented` check, so the React issue 9023 workaround guard is dropped. A controlled consumer that calls `event.preventDefault()` in `onValueChange` and still updates its state now gets clearErrors and validation through `useValueChanged`, which the old code suppressed. That is a behavior change nobody mentioned, and no test covers it, because the existing "change is prevented" test is uncontrolled. The remedy is to fold the guard into the shared helper from finding 1. That means computing one `shouldSync = !defaultPrevented && !details.isCanceled` at the top of the uncontrolled branch and applying it to the dirty and filled updates as well. It also means either documenting the controlled-mode exemption or testing it.

### 3. Filled state now has three writers

`setFilled` is now called from the mount layout effect (lines 99-103), from the `useValueChanged` callback (line 112), and from `onChange` (line 150). The mount effect reads the DOM to cover the uncontrolled `defaultValue` case, and in the controlled case it duplicates what `serializedValue` already says. Before this change the equivalent logic was a single effect keyed on `valueProp`. A cleaner shape derives the mount-time value once (`serializedValue ?? inputRef.value`) and feeds the same helper as findings 1 and 2, so that filled has one writer per trigger and no separate DOM-peeking effect. The PR body concedes that the mount-time DOM read is a residual difference from sibling controls. It should at least be isolated behind the helper and named, instead of left as an anonymous layout effect between two other hooks.

### 4. `String(value)` is a quiet contract at the registration boundary

Line 86 turns any controlled value, including arrays and numbers permitted by the `input` value typing, into a string with `String(value)`. That string is registered as the field's control value, and it is also the baseline for dirty comparison. For numbers this is exactly right. For arrays, `['a','b']` becomes `"a,b"`, which matches what the DOM shows but not what the consumer holds. Anything reading the registration `value` (form value collection and reset) now sees a string where it used to see the original value. It is not clear that every reader tolerates that, and the PR does not add a test on the Form side. This is the right fix for the dirty bug, but the assumption "registered value equals DOM string" should be stated in the registration type or covered by a Form-level test rather than being implied by an inline comment.

### 5. Diff noise and test gaps

The diff moves the `inputRef` declaration and the autofocus effect below the new hooks with no functional reason, which makes the review harder for no gain. The new tests cover the happy paths well (numeric dirty, programmatic set, prefilled mount, cancel in uncontrolled mode). Missing are a controlled value the consumer rejects or rewrites, which the PR advertises as a fix, the controlled `preventDefault` case from finding 2, and dirty and filled state after a canceled uncontrolled change. The render-count test is titled "renders once per keystroke" while the PR table documents two renders per keystroke in `onChange` mode. The title and the documented cost should agree.

## Accepted trade-off

Moving the work into a layout effect adds one render per keystroke in `onChange` validation mode. The author documents this and matches Number Field. I would not block on it. It is the price of one sync primitive and a consistent shape across controls.

## Proposed remediation sequence

First, extract the shared sync helper (findings 1 and 3) so that dirty, filled, clearErrors and validate live in one function. Second, apply the cancel and `preventDefault` guard to the whole uncontrolled sync, and decide and document the controlled `preventDefault` semantics (finding 2). Third, add tests for a rejected or rewritten controlled value, controlled `preventDefault`, and canceled uncontrolled dirty state, and a Form-level test for the serialized registration value (findings 4 and 5). Fourth, restore the original ordering of `inputRef` and the autofocus effect to shrink the diff.

## Verification status

I ran `FieldControl.test.tsx` under jsdom and it passes. I did not run the Form or sibling control suites. The behavior claims in findings 2 and 4 come from reading the code paths (`FieldControl.tsx`, `useValueChanged.ts`, `useRegisterFieldControl.ts`, `FieldRoot.tsx` `setDirty`). I did not exercise them with scratch tests, so they are unverified by execution.
