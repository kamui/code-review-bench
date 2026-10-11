# 02 — Render cost, test claims, and the PR description

Scope: `packages/react/src/field/control/FieldControl.test.tsx` at head `14d39e5d1`, the pull-request body in the review packet, and the render behaviour of the controlled path in `FieldControl.tsx`. Method and variants (head / base / judo) are described at the top of `01_field-control-sync.md`.

## Finding B — the controlled sync now costs a second commit per keystroke whenever validation publishes

At the merge-base, a keystroke in a controlled control did all of its field work inside the React change handler: `onValueChange` (the consumer's `setState`), `setDirty`, `setFilled` and `validation.change` ran in one event, and React batched them into one render.

At head the handler returns early for controlled values (`FieldControl.tsx:142-146`) and the field work runs from `useValueChanged` (`FieldControl.tsx:105-115`), which is a layout effect. The order is now: consumer `setState` → render → commit → layout effect → `validation.change` → `setValidityData` → a second, synchronous render and commit. `validityData` is part of the `FieldRoot` context value, so that second pass re-renders `Field.Root` and every part that reads the context (label, description, error, control).

The PR anticipates this and adds a test, `FieldControl.test.tsx:47-80`, "renders once per keystroke for controlled input changes", with the comment "The controlled echo must not schedule a second render per keystroke." The test passes, but only because of the mode it runs in. It renders `<Field.Root>` with no `validationMode`, so the mode is `onSubmit` with no submit attempt, `shouldValidateOnChange()` is false, `change` calls `commit(value, true)`, and the revalidate branch returns at `useFieldValidation.ts:170-172` before any state is set. Nothing is published, so there is nothing to re-render.

Probe H1 is the same test with `validationMode="onChange"` and a validator returning `null`. Renders of the control for two steady-state keystrokes:

| variant | renders | `validate` calls (3 keystrokes) |
| --- | --- | --- |
| base | 2 | 3 |
| head | **4** | 3 |

Probe P4 uses `validationMode="onBlur"`, a validator that fails short values, one keystroke, a blur (field becomes invalid), then two more keystrokes:

| variant | first keystroke after invalid | second |
| --- | --- | --- |
| base | 1 | 1 |
| head | **2** | 1 |

So the second render is back in every case where `commit` actually publishes: always under `validationMode="onChange"`, after the first submit attempt under the default `onSubmit` mode, and on the keystroke that clears an existing error in any mode. Validation itself still runs once per keystroke; this is a render-count regression, not a double validation.

This is a real trade and the head's side of it is defensible: "a controlled control syncs from its prop, full stop" is a simpler model than the one the PR body describes, and it is the model every sibling control uses. But the test and its comment claim something the code does not deliver, and the claim is exactly the one a later reader will rely on.

Remediation, in order of preference:

1. Make the test state what is true. Parameterise it over `validationMode` (`'onSubmit'`, `'onChange'`) and assert the real counts, so the cost is pinned and visible instead of hidden behind a mode where validation is a no-op.
2. If two commits per keystroke on a text input is not acceptable — text inputs are the one control where this path runs at typing speed — keep the synchronous work in the handler and let the effect handle only values the handler has not already processed. That is the `processedValueRef` design the PR body describes and the head does not contain. It costs one ref and one comparison, and with the single `syncFieldState` function from `01_field-control-sync.md` it is a guard at one call site rather than a second pipeline.

The benchmark comment in the packet does not bear on this. It was produced for commit `6a40cf1`, not the head, and the flagged tests (Menu, Dialog, Tooltip mount) do not touch `Field`.

## Finding C — the description is for a different implementation

The packet's bot comments all reference commit `6a40cf1`. The head is `14d39e5d1`. The body was evidently written for the earlier push and not revised. Checked claim by claim against the head:

| PR body says | Head |
| --- | --- |
| "`processedValueRef` records the value the change handler just handled, so the prop echo does not validate the same keystroke twice." | No such identifier exists anywhere under `packages/react/src` (`grep -rn processedValueRef` returns nothing). The head avoids double validation by returning early from the handler instead. |
| "A control that unmounts or becomes disabled while focused no longer latches `data-focused`." | The diff does not touch `onFocus`, `onBlur`, `setFocused`, or add any cleanup. Probe P2 focuses the control, disables it, then unmounts it: `data-focused` on `Field.Root` is present before, after disabling, and after unmounting, for head and base alike. |
| "The sibling controls also call `clearErrors(name)`… This PR does not." (Open question) | `FieldControl.tsx:110` calls `clearErrors(name)` in the controlled sync. Probe P3 sets a controlled value from a button under a `Form` with a server error: base keeps the error, head clears it. |
| "`details.cancel()` in `onValueChange` now stops the internal handling." | Uncontrolled: it stops `clearErrors` and `validation.change` but not `setDirty` / `setFilled`. Controlled: it has no effect. See Finding A in `01_field-control-sync.md`. |

Probe P2 ran under jsdom, where neither disabling nor removing a focused element dispatches `blur`. That is the scenario the body describes ("no blur fires in that path"), but it was not confirmed in a real browser.

Why this matters beyond tidiness: there is no linked issue, so the body is the only statement of intent for this change, and it is what a reviewer or a later reader of the merged PR will take as the record. As written, it announces a focus fix that is not in the diff, describes a guard that does not exist, and presents a design question as open when the code has already answered it.

The tests have the same gap. Of the behaviours the head actually changes, these have no assertion:

- A programmatic change to a controlled value clears a `Form` error (the resolved open question). None of the new tests renders a `Form`.
- A natively prevented change event on a *controlled* control now clears errors and validates (probe P1). The existing prevented-event test at `FieldControl.test.tsx:203-226` is uncontrolled only.
- `cancel()` on an uncontrolled control still flips `data-dirty` / `data-filled` (probe H2).
- The render count under `validationMode="onChange"` (Finding B).

Remediation: rewrite the body to describe the head; either restore the focus-latch fix with a test or remove the claim and file it separately; state the `clearErrors` decision as a decision and add the `Form`-error test that pins it.

## Test-file notes that are not findings

- The rewrite of "avoids rerendering for uncontrolled input changes" (`FieldControl.test.tsx:19-45`) is an improvement. The old version counted renders of a wrapper component whose element identity never changed, so it could not observe the control re-rendering; counting inside the `render` callback measures the thing the test name describes.
- "clears dirty state when a numeric controlled value returns to its initial value" (`:116-140`) is a good regression test for the baseline fix. Probe H9 reproduces the bug at base and its absence at head.

## Open questions for the author

**Late-loaded values.** Probe H6 mounts a controlled control with `''`, then sets `'loaded'` from a mount effect under `validationMode="onChange"` with a failing validator, with no user interaction:

| variant | `data-dirty` | `data-invalid` | `validate` calls |
| --- | --- | --- | --- |
| base | no | no | 0 |
| head | yes | yes | 1 |

The baseline is captured once, at first registration (`useFieldControlRegistration.ts:91-102`), so any form that hydrates controlled text values after mount now comes up dirty, and under on-change validation comes up showing errors for data the user never touched. The new test at `FieldControl.test.tsx:142-167` asserts `data-dirty` after a programmatic set, so this is by design, and it matches the sibling controls. Text inputs are where fetch-then-set is most common, though, so it deserves an explicit yes.

**Prevented events when controlled.** Probe P1 above. Is the react#9023 workaround meant to apply only to uncontrolled controls from now on?
