# Thermo-nuclear code quality review: mui/base-ui#5460 (Field.Control controlled sync)

Range reviewed: `30b8ea2004fa..14d39e5d1ad6` (one commit, 2 files, +197/−26). Only one subsystem is touched, so there is a single detail file: `01_field-control.md`. The PR was already merged; this review is retrospective and no edits were made.

## Verdict

Not approvable as-is on the strict bar, though the direction is right. Moving `Field.Control` onto `useValueChanged` makes it match its nine siblings, and the "one owner per mode" framing is the correct model. The file stays small (207 lines) and nothing crosses the 1k-line threshold. The trouble is that the implementation stops halfway. The two owners still carry near-identical copies of the sync sequence. The uncontrolled path keeps a partially applied "cancel". The controlled path silently drops an existing guard. A third mount-time path also survives.

I ran the existing `FieldControl.test.tsx` (26 passed, 1 skipped). I also ran two scratch probes outside the clone. They confirm the behavioral findings F1 and F2 below.

## Findings

### F1. Controlled mode no longer honours a prevented change event

In `FieldControl.tsx` the uncontrolled `onChange` still gates `clearErrors` and `validation.change` on `!event.nativeEvent.defaultPrevented`, with a comment pointing at the React issue 9023 workaround. The new controlled path (`useValueChanged`, lines 105-115) has no equivalent. It runs `clearErrors`, `setDirty`, `setFilled` and `validation.change` whenever the string value changes. A scratch probe (controlled input, `validationMode="onChange"`, capture listener calling `preventDefault()` on the `input` event, `onValueChange={setValue}`) produced one `validate` call. Base and the uncontrolled path both produce zero for the same event. The existing "does not clear errors or validate when change is prevented" test is uncontrolled only, so nothing pins this. This is an unannounced behavior change hidden inside a refactor. Either carry the prevented state into the controlled path (for example by recording it in a ref that the effect consults) or state explicitly, in the PR and in a test, that controlled mode ignores `defaultPrevented`. The workaround comment also now describes only half of the component.

### F2. `details.cancel()` stops validation but not dirty/filled in the uncontrolled path

The PR states that `details.cancel()` now "stops the internal handling". In `onChange` (lines 148-156) `setDirty` and `setFilled` run unconditionally before the `defaultPrevented`/`isCanceled` check, and only `clearErrors` and `validation.change` are gated. A scratch probe with an uncontrolled `Field.Control` whose `onValueChange` calls `details.cancel()` shows `data-dirty` and `data-filled` both set after the change. The added test asserts only that `validate` is not called, so it passes. This leaves a half-applied update: validity ignores the change while dirty and filled reflect it. That is the non-atomic shape the skill warns about. Move the cancel and prevented guard to the top of the uncontrolled branch so the whole sync block is all-or-nothing, and extend the test to assert dirty and filled. If the intent is that the DOM has already changed, so dirty/filled must track it, then the claim in the PR body and the test name should say "cancel skips validation" and no more.

### F3. The sync sequence is duplicated across the two owners (missed code-judo move)

`clearErrors(name); setDirty(x !== (validityData.initialValue ?? '')); setFilled(x !== ''); validation.change(x)` now appears twice, once in the `useValueChanged` callback and once in `onChange`. They differ only in ordering, guards and where the value comes from. That is exactly how F1 and F2 arose: each copy has drifted into its own guard set. The cleaner structure is a single local function, for example `syncFieldState(nextValue: string)`, that owns the whole sequence including the "update dirty before validating because `validation.change` reads `markedDirtyRef`" ordering rule. The controlled effect and the uncontrolled `onChange` would then each be one line, and the guards would live in one place. Because the PR's own thesis is "one owner per mode", the deliverable should be one implementation with two triggers, not two implementations.

### F4. A third, asymmetric mount-time filled path remains

The layout effect at lines 99-103 sets `filled` from the DOM only when non-empty and never clears it. `useValueChanged` does not fire on mount, so that effect is the only thing covering initial state, and it now overlaps with the controlled effect's `setFilled(serializedValue !== '')`. The PR describes this as the "residual difference" required by uncontrolled mode, but it also runs in controlled mode, where `serializedValue` is already known. Restrict it to the uncontrolled case, or express both as "initial filled = DOM value is non-empty" in the shared helper from F3. Otherwise a reader has to work out three separate places that write `filled`.

### F5. The `serializedValue` boundary is an implicit cast with a duplicated null guard

`value == null ? undefined : String(value)` (line 86) turns `undefined` into "not controlled" and `null` into "skip". The callback then re-checks `serializedValue === undefined` (line 106) to skip. A controlled `value={null}` therefore registers as uncontrolled and is silently ignored by sync. Objects and arrays become `String(...)` output such as `"a,b"` or `"[object Object]"`, and the registration now stores the string rather than the consumer's value. I did not verify whether any public API (for example `Field.Validity` data) exposes `initialValue`, so I cannot say whether that is observable. State the intended contract explicitly, for example by typing the prop as `string | number | readonly string[]` and handling `null` at one point, instead of relying on `String()` coercion plus a guard inside the callback.

### F6. Unrelated hook reordering enlarges the diff

The `inputRef` declaration and the autoFocus layout effect were moved below the new hooks. Nothing about the change requires that, and it makes review harder. The `useIsoLayoutEffect` for `filled` also moved and changed shape at the same time. Keep unrelated hooks in place.

### F7. Test coverage does not pin the claims the PR makes

The new "renders once per keystroke" test measures only the default `onSubmit` mode. The PR itself reports two renders per keystroke in `onChange` mode, and that trade-off is untested and undocumented in code. There is no test for the stated fix that a pending async validator for the old value no longer publishes. There is none for a controlled value the consumer rejects or rewrites never reaching field state, and none for programmatic reset clearing a visible error. Nothing covers cancel for controlled mode or the prevented event for controlled mode (see F1). Add those tests, since each is a named behavior in the PR description.

## Proposed remediation sequence

1. Extract a single `syncFieldState(nextValue)` helper (F3) that owns clearErrors, dirty, filled and validate in the correct order.
2. Decide and encode the cancel and prevented semantics in that one place for both modes (F1, F2), and update the test names to match.
3. Fold or restrict the mount-time filled effect (F4) and restore hook order (F6).
4. Make the controlled value contract explicit at one boundary (F5).
5. Add the missing tests (F7), starting with the two behaviors already confirmed by probe.

Full evidence, the probe sources and worked proposals are in `01_field-control.md`.
