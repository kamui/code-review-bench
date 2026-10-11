# Thermo-nuclear code quality review: mui/base-ui#5460 "[field] Sync controlled value changes with field state"

Range: `30b8ea2004fa999bed151204208676c6c0a9d261..14d39e5d1ad6b7aca2fb067415dba09c6bea219b`, 2 files, +197/-26. Reviewed by one primary reviewer (claude-sonnet-5-5, high). No other reviewer or model was used. Detail: [01_field-control.md](01_field-control.md).

## Verdict

Not approvable as structured. The direction is right: `Field.Control` was the odd one out among field controls, and routing controlled values through `useValueChanged` like Switch, NumberField, and the others is the canonical move. Neither file approaches the 1k-line limit (207 and 273 lines). The problem is how the change was done. It leaves two parallel copies of the same four-step state-sync pipeline joined by an `isControlled` early return, and in the process the controlled path silently drops two behaviors the uncontrolled path has (the prevented-change guard and the cancel contract). I confirmed one of those regressions with a scratch test against the head source. The existing field suite passes (10 files, 240 passed, 9 skipped), and CI could not catch the regression because the tests that cover prevented and canceled changes are uncontrolled-only.

## Findings

**Finding 1: the same state-sync sequence now lives in two places, held apart by an `isControlled` early return.** In `packages/react/src/field/control/FieldControl.tsx`, the `onChange` handler (lines 137-157) still runs `setDirty`, `setFilled`, `clearErrors`, and `validation.change`, and the new `useValueChanged` block (lines 105-115) runs the same four calls in the same order. That order is load-bearing, because `validation.change` reads `markedDirtyRef`, yet the explanatory comment exists only on the DOM path. The two copies are kept from double-firing by `if (isControlled) return;` in the handler and `if (serializedValue === undefined) return;` in the effect, which is a mode split pushed down into the middle of a busy handler. The code-judo move is to extract one `syncFieldState(next: string)` stable callback that owns the ordering and the comment, and call it from both places, so that the handler reads "call `onValueChange`, then sync unless controlled, canceled, or prevented". That deletes the duplicated sequence, leaves one place to change semantics, and makes Findings 2 and 3 one-line fixes. Full proposal in the detail file.

**Finding 2: controlled mode loses the `defaultPrevented` guard, and a scratch test confirms the regression.** For uncontrolled inputs, a default-prevented input event skips `clearErrors` and `validation.change` (the react#9023 workaround branch at line 153). The controlled early return at line 144 bypasses that branch entirely, and the prop echo then validates unconditionally. I ran a controlled `Field.Control` with `validationMode="onChange"` and a capture-phase `preventDefault()` on the input event: the validator was called once, where the repo's own uncontrolled test expects zero calls. The new tests cover prevented changes only for uncontrolled inputs, so this passes CI. Either carry the "prevented" fact to the effect (a small ref, which is what the description's `processedValueRef` was apparently for) or state in the docs and code that controlled mode intentionally ignores it.

**Finding 3: `details.cancel()` is honored for only one of four internal updates, and is bolted onto an unrelated workaround.** `!details.isCanceled` is appended to the `defaultPrevented` condition at line 153, joining a React bug workaround and a public API contract in one boolean. It guards `clearErrors` and `validation.change` but not `setDirty` or `setFilled`: a scratch test shows a canceled uncontrolled change still sets `data-dirty` and `data-filled`. In controlled mode, `cancel()` followed by a state update still validates through the echo. The description says cancel "stops the internal handling", which is not what the code does. Decide the contract (cancel means "leave field state alone" versus "do not validate"), give it its own early `if` right after `onValueChange`, and document it on the `onValueChange` prop. With Finding 1's helper that is a single guard.

**Finding 4: `serializedValue` hides a lossy conversion and overloads `undefined`.** `value == null ? undefined : String(value)` (lines 85-86) turns numbers and arrays into strings for the baseline and for validation, and collapses `null` into "not controlled", so `value={null}` is never synced. The `undefined` check in the effect therefore means two different things at once (uncontrolled, and controlled but nullish). Array values become `'a,b'`, which is correct for an `<input>` but is pinned by no test, even though the description calls arrays out. Give the conversion a named helper with a one-sentence contract (or narrow the prop type), and add tests for `null` and array values. Because the registration `value` type is `unknown`, the compiler cannot catch a mismatch for you.

**Finding 5: the diff moves unrelated code.** The `inputRef` declaration and the autoFocus effect are relocated below the new hook (lines 117-123), although nothing in the change requires it (`inputRef` is first used in `useRenderElement`). This inflates the diff and moves `git blame` for the autoFocus effect onto this PR. Leave the block where it was. Low priority, but free to fix.

## Test-suite notes

The rewrite of the "avoids rerendering" test is a real improvement: the old wrapper component never re-rendered with `Field.Control`, so its assertion was vacuous. The new "renders once per keystroke" test asserts an exact count (`settled + 2`) and will be brittle to unrelated render changes. Missing coverage: controlled plus prevented (Finding 2), controlled plus cancel (Finding 3), `null` and array values (Finding 4), a consumer that rewrites or rejects the typed value (the code comment at lines 142-143 relies on this), and the stale async-validator scenario named as a motivating bug in the PR description.

## Questions for the author

**Question 1: which version of the change is intended, the description's or the diff's?** The description mentions a `processedValueRef` guard and a fix for `data-focused` latching on unmount or disable, and says the programmatic path does not call `clearErrors(name)`. The diff contains neither of the first two and does call `clearErrors(name)` in `useValueChanged` (line 110), so the description's "Open question" is already answered the opposite way in code.

**Question 2: is it intended that controlled-mode validation moves from the keystroke to the consumer's next committed render?** A consumer that debounces or batches its state now delays validation, dirty, and filled with it. The code comment presents that as the point of the change ("a value the consumer rejects or rewrites never reaches the field state"); if so, `Field.Control`'s docs should say it.

## Proposed remediation sequence

1. Extract `syncFieldState` (Finding 1) and call it from both the handler and the `useValueChanged` effect, keeping the "dirty before validate" comment on it.
2. Decide and encode the cancel contract as one early guard after `onValueChange` (Finding 3), separate from the react#9023 workaround.
3. Carry the prevented-change fact to the controlled echo, or document that controlled mode ignores it (Finding 2), and add controlled tests for prevented and canceled changes.
4. Name the serialization helper and add `null` and array tests (Finding 4).
5. Restore the original position of the `inputRef`/autoFocus block (Finding 5).
6. Reconcile the PR description with the code (Question 1).

Verification status: findings 2 and 3 were reproduced with scratch tests run offline from the work directory against the head source; findings 1, 4, and 5 are by reading the diff and the sibling implementations (`SwitchRoot.tsx:105`, `NumberFieldInput.tsx:101`). Nothing in the clone was modified.
