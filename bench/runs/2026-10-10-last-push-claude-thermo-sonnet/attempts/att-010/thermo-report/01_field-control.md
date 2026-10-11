# Detail: `Field.Control` (packages/react/src/field/control/FieldControl.tsx and its test)

Range reviewed: `30b8ea2004fa..14d39e5d1ad6` (`git diff main...review-head`), 2 files, +197/-26. `FieldControl.tsx` is 207 lines after the change, and the test file is 273 lines, so the 1k-line threshold is not a concern.

## Measurements and commands

- `git diff main...review-head -- packages/react/src/field/control/FieldControl.tsx` was read in full. The component now has two parallel state-sync paths: the `onChange` handler at lines 137-157 and the `useValueChanged(serializedValue, ...)` block at lines 105-115.
- Existing suite: `TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field`. Result: 10 files, 240 passed, 9 skipped, 0 failed.
- Scratch probes (in `clone-work/scratch`, outside the clone, using the alias config from the execution policy) were run against the head source:
  1. Controlled `Field.Control` with `validationMode="onChange"`, a `validate` spy, and a capture-phase `input` listener calling `preventDefault()`. Result: `validate` was called **1** time. The equivalent uncontrolled test in the repo (`does not clear errors or validate when change is prevented`) expects 0 calls.
  2. Controlled control whose `onValueChange` calls `details.cancel()` and then `setValue(x)`. Result: `validate` was called **1** time.
  3. Uncontrolled control whose `onValueChange` calls `details.cancel()`. Result: root has both `data-dirty` and `data-filled`, so cancel does not stop the dirty/filled updates.
  4. `value={null}` mounts without error (it silently takes the "uncontrolled-looking" `undefined` registration path).
- Siblings compared: `SwitchRoot.tsx:105-111` and `NumberFieldInput.tsx:101-110` use `useValueChanged` as their only sync path.

## Finding 1: two copies of the same sync sequence, linked by an `isControlled` early return

The change handler still does `setDirty`, `setFilled`, `clearErrors`, `validation.change` (lines 148-156). The new `useValueChanged` block does the same four things in the same order (lines 110-114). The ordering is load-bearing, because `validation.change` reads `markedDirtyRef`. The comment that explains this ("update dirty before validating") sits only on the DOM path. The effect path keeps the same ordering by coincidence of copy-paste.

Between the two copies, `if (isControlled) return;` at line 144 decides which one runs. The result is a mode split inside a handler: for controlled inputs the handler is just `onValueChange`, and for uncontrolled inputs it is the full pipeline. The `serializedValue === undefined` guard at lines 106-108 is the mirror image of that split, and the pair exists only to keep the two paths from both firing.

### Worked code-judo proposal

Extract one local function and call it from both places.

```ts
const syncFieldState = useStableCallback((next: string) => {
  clearErrors(name);
  // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
  setDirty(next !== (validityData.initialValue ?? ''));
  setFilled(next !== '');
  validation.change(next);
});
```

Then `onChange` becomes: call `onValueChange`, and if `!isControlled && !details.isCanceled && !event.nativeEvent.defaultPrevented`, call `syncFieldState(inputValue)`. The effect becomes `useValueChanged(serializedValue, () => { if (serializedValue !== undefined) syncFieldState(serializedValue); })`. The sequencing invariant lives in one place, and a future change (such as aligning the cancel/prevented semantics, below) happens once.

A further step is to make the DOM `change` event the only trigger for user edits in both modes and to have the controlled prop effect cover only the programmatic delta. That needs the `processedValueRef` guard that the PR description mentions. The diff does not contain it (see Question 1).

Status: verified by reading and by the scratch probes. Not applied.

## Finding 2: controlled mode loses the `defaultPrevented` guard (verified behavior regression)

In uncontrolled mode, `clearErrors` and `validation.change` are skipped when the native event was default-prevented. That is the existing "Workaround for react#9023" branch. In controlled mode the early return at line 144 skips that branch entirely, and the prop echo then runs `clearErrors` and `validation.change` unconditionally. Scratch probe 1 shows `validate` is called once for a prevented input event on a controlled control, where the uncontrolled path calls it zero times. The new test suite covers prevented changes only in the uncontrolled form (the pre-existing test), so the regression is invisible to CI.

Remedy: the prevented-change check is information about the DOM event, so a controlled-mode echo cannot see it. Either record it in a ref that the effect consults (the `processedValueRef` idea) or document that controlled mode deliberately ignores `preventDefault` and delete the dead-looking workaround comment for that mode. The first option keeps the two modes consistent.

Status: CONFIRMED by scratch probe 1.

## Finding 3: `details.cancel()` is honored only partly, and the guard is stacked onto an unrelated workaround

`!details.isCanceled` is appended to the react#9023 `defaultPrevented` condition (line 153). That joins two unrelated concerns, a React bug workaround and the public cancel contract, in one boolean. It also guards only `clearErrors` and `validation.change`. `setDirty` and `setFilled` still run above it (lines 149-150). Probe 3 shows an uncontrolled canceled change still sets `data-dirty` and `data-filled`. Probe 2 shows that in controlled mode, `cancel()` followed by a state update still validates through the echo. The PR description says cancel "stops the internal handling", but in practice it stops one of four internal effects in one of two modes.

Remedy: decide the contract. If cancel means "do not touch field state", return right after `onValueChange` when `details.isCanceled`, before any `setDirty`/`setFilled`, and note that the DOM value of an uncontrolled input can then diverge from field state. If cancel means "do not validate", say so in the doc comment on `onValueChange`. Put the check in its own early `if`, separate from the react#9023 guard. With Finding 1's helper this is a one-line decision rather than three scattered conditions.

Status: CONFIRMED by scratch probes 2 and 3 (behavior), design judgment otherwise.

## Finding 4: `serializedValue` quietly narrows the boundary with `String()` and a nullish fallback

`const serializedValue = value == null ? undefined : String(value)` (lines 85-86) changes what `useRegisterFieldControl` stores as the baseline and what `validation.change` receives. A few consequences follow.

- `null` and `undefined` collapse into "no controlled value", so `value={null}` stops being synced at all (probe 4). The `serializedValue === undefined` guard is then doing double duty as "not controlled" and "controlled but nullish".
- Array values (`readonly string[]` is allowed by the prop type) become `'a,b'`. That matches what `<input>` does with `String(array)`, but no test pins it, and the PR text singles out arrays.
- The registered value passed to the form registration now differs in type from the old behavior (number or array became string). Form submission uses `getValueFromInput`, so this looks safe, but the type of `FieldControlRegistration['value']` is `unknown`, so the compiler cannot check it.

Remedy: add a small named helper (for example `serializeControlValue(value): string | undefined`) with a one-line contract, or narrow the `value` prop type for `Field.Control` so the cast-by-`String()` disappears. Add tests for `null` and array values.

Status: PLAUSIBLE (design). Probe 4 confirms the `null` path does not crash, and does not exercise sync.

## Finding 5: unnecessary diff churn that moves unrelated code

The `inputRef` declaration and the autoFocus layout effect are moved below the new hook (lines 117-123), and the old filled-sync effect is rewritten in place. The move is not needed by anything in the change: `inputRef` is first used inside `useRenderElement`. It makes the diff look larger than the logic change and makes `git blame` on the autoFocus effect point at this PR. Keep the moved block where it was.

Status: CONFIRMED by reading the diff. Low priority.

## Test-suite observations

- The rewrite of `avoids rerendering for uncontrolled input changes` replaces a wrapper component that did not re-render with `Field.Control` (so the old assertion was vacuous) with a `render`-prop counter. That is an improvement. The new `renders once per keystroke` test asserts an exact count (`settled + 2`), which is sensitive to unrelated render changes. Consider `toBeLessThanOrEqual`.
- Missing coverage: controlled + prevented (Finding 2), controlled + cancel (Finding 3), array and `null` values (Finding 4), controlled consumer that rewrites or rejects a value (the code comment at lines 142-143 makes this claim, but nothing tests it), and the stale async-validator scenario that the PR description lists as a motivating bug.

## Questions

### Question 1: the PR text and the diff disagree

The description names a `processedValueRef` guard, a fix for `data-focused` latching on unmount or disable, and says `clearErrors(name)` is *not* called on the programmatic path. None of the first two are in the diff, and the diff does call `clearErrors(name)` in `useValueChanged` (line 110). The "Open question" in the description is therefore already answered by the code in the opposite direction. Which version is intended?

### Question 2: should controlled-mode DOM edits validate on the keystroke or on the echo?

The change moves controlled validation from the DOM event to the next layout effect after the consumer's state update. A consumer that debounces or batches its state therefore also delays validation and the dirty/filled flags. The code comment frames this as a feature ("a value the consumer rejects ... never reaches the field state"). If so, it should be stated in the docs for `Field.Control`.
