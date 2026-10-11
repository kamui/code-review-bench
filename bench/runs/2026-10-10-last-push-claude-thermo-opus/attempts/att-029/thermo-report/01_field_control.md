# 01 — `Field.Control` controlled-value sync (detail)

Subsystem: `packages/react/src/field/control/FieldControl.tsx` (+48 / −20) and its test file (+149 / −6). Summary: [`summary.md`](summary.md).

## How the evidence was produced

The clone was read at `review-head` (`14d39e5d1`) and diffed with `git diff main...review-head`. Nothing in the clone was changed; `git status --short` was empty after the runs.

The existing suite was run once at the head:

```
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react \
  packages/react/src/field/control/FieldControl.test.tsx
→ Test Files 1 passed (1) · Tests 26 passed | 1 skipped (27)
```

Behavioural claims were then probed in a scratch directory outside the clone (`clone-work/scratch`), set up as the execution policy describes: `node_modules` linked to `packages/react/node_modules`, a `vitest.config.mts` with jsdom and aliases from `@base-ui/react/<name>` and `@base-ui/utils/<name>` to the clone's sources, and `@testing-library/react` for rendering. One probe file, `probe.test.tsx`, runs every scenario against three controls inside the clone's own `Field.Root`:

- **base** — `git show main:packages/react/src/field/control/FieldControl.tsx`, with only its relative imports rewritten to absolute paths into the clone;
- **head** — `Field.Control` as exported from the clone;
- **alt** — the head file with the two edits proposed under findings 1 and 3 applied, and nothing else.

```
TZ=UTC <clone>/node_modules/.bin/vitest run --root <clone-work>/scratch
→ Test Files 1 passed (1) · Tests 31 passed (31)
```

The probes record observations instead of asserting them; the table below is the recorded output (`scratch/probe-output.txt`). The first run used an inline `errors={{…}}` object on `Form`, which `Form` re-applies on every parent render and which masked the `clearErrors` result in the controlled scenarios; the numbers below are from the rerun with a module-level constant.

| Probe | base | head | alt |
| --- | --- | --- | --- |
| T1 renders for 2 steady-state controlled keystrokes, `validationMode="onChange"` | 2 | **4** | 4 |
| T1 renders for 2 steady-state controlled keystrokes, default `onSubmit` | 2 | 2 | 2 |
| T1 `validate` calls for 3 controlled keystrokes, `onChange` | 3 | 3 | 3 |
| T2 controlled, handler calls `cancel()` then `setValue(next)`: `validate` calls | 1 | 1 | 1 |
| T2 same: server error still shown | no | no | no |
| T2b uncontrolled, handler calls `cancel()`: `validate` calls | 1 | 0 | 0 |
| T2b same: server error still shown | no | yes | yes |
| T2b same: `data-dirty` / `data-filled` on the root | yes / yes | yes / yes | yes / yes |
| T3 `data-filled` after a filled control is swapped for an empty controlled one | no | **yes** | no |
| T4 `data-focused` after the focused control unmounts | yes | yes | yes |
| T4 `data-focused` after the focused control becomes disabled | yes | yes | yes |
| T6 `Form` server error after a programmatic clear of the controlled value | shown | **cleared** | cleared |
| T6 `data-dirty` after that programmatic clear | no | yes | yes |
| T7 numeric `value={5}`: dirty after `56`, after returning to `5` | yes, yes | yes, no | yes, no |
| T8 controlled change the consumer rejects: `validate` calls / dirty / filled | 1 / yes / yes | 0 / no / no | 0 / no / no |
| T9 controlled value set to `null`: DOM text / `data-filled` | `abc` / yes | `abc` / yes | `abc` / yes |

T7 and T8 confirm the two things the PR sets out to fix. They are fixed, and the fix is the right shape.

## Finding 1 — two sync bodies, two contracts

**Where.** `FieldControl.tsx:105-115` (prop path) and `FieldControl.tsx:137-157` (event path).

**What the head has.**

```tsx
useValueChanged(serializedValue, () => {
  if (serializedValue === undefined) {
    return;
  }

  clearErrors(name);
  setDirty(serializedValue !== (validityData.initialValue ?? ''));
  setFilled(serializedValue !== '');

  validation.change(serializedValue);
});
```

```tsx
onChange(event) {
  const inputValue = event.currentTarget.value;
  const details = createChangeEventDetails(REASONS.none, event.nativeEvent);
  onValueChange?.(inputValue, details);

  // Controlled values sync from the `value` prop instead, so that a value the consumer
  // rejects or rewrites never reaches the field state.
  if (isControlled) {
    return;
  }

  // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
  setDirty(inputValue !== (validityData.initialValue ?? ''));
  setFilled(inputValue !== '');

  // Workaround for https://github.com/react/react/issues/9023
  if (!event.nativeEvent.defaultPrevented && !details.isCanceled) {
    clearErrors(name);
    validation.change(inputValue);
  }
},
```

The same four calls, the same two expressions (`!== (validityData.initialValue ?? '')`, `!== ''`), in a different order, with the ordering comment on only one of them and the cancel guard on only one of them. A reader has to diff the two blocks by eye to learn that they are meant to be the same operation, and the next person to change one will not be told about the other.

**Why the divergence is more than cosmetic.** T2 and T2b show the contract split. Uncontrolled plus `cancel()`: no validation, server error kept, but dirty and filled flip — the update is half-applied. Controlled plus `cancel()` plus `setValue`: the prop path has no way to know the change was canceled, so it validates and clears the server error. Neither is obviously wrong in isolation; having both in one component under one event-details API is.

**Worked proposal (behaviour-preserving, run as `alt`).**

```tsx
const syncFieldState = useStableCallback((nextValue: string, shouldValidate = true) => {
  // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
  setDirty(nextValue !== (validityData.initialValue ?? ''));
  setFilled(nextValue !== '');

  if (shouldValidate) {
    clearErrors(name);
    validation.change(nextValue);
  }
});

useValueChanged(serializedValue, () => {
  if (serializedValue !== undefined) {
    syncFieldState(serializedValue);
  }
});
```

```tsx
onChange(event) {
  const inputValue = event.currentTarget.value;
  const details = createChangeEventDetails(REASONS.none, event.nativeEvent);
  onValueChange?.(inputValue, details);

  // Controlled values sync from the `value` prop instead, so that a value the consumer
  // rejects or rewrites never reaches the field state.
  if (!isControlled) {
    // Workaround for https://github.com/react/react/issues/9023
    syncFieldState(inputValue, !event.nativeEvent.defaultPrevented && !details.isCanceled);
  }
},
```

One body, one ordering comment, no mid-handler return. The `shouldValidate` flag exists only to preserve the head's half-applied behaviour for a prevented or canceled uncontrolled change. If the maintainers decide that a canceled uncontrolled change should not move dirty and filled either, the flag disappears and the handler becomes `if (!isControlled && !event.nativeEvent.defaultPrevented && !details.isCanceled) syncFieldState(inputValue);` — the simplest form, but a behaviour change relative to the merge-base (which also flipped dirty and filled on a prevented change), so it is offered as a decision, not as part of the refactor.

**Verification status.** The proposal was applied to a copy of the head file and run through all probes: every `alt` cell equals the corresponding `head` cell except T3, which differs because `alt` also carries the finding 3 fix. It was **not** run against the repository's own `FieldControl.test.tsx` (that would have required changing the clone); the probes re-create that file's numeric-dirty, programmatic-sync, validate-once, cancel and render-count scenarios.

## Finding 2 — a second commit per controlled keystroke, and a test that says otherwise

**Where.** `FieldControl.tsx:105-115`; `FieldControl.test.tsx:47-80` (the comment is at line 78).

**Mechanism.** At the merge-base, a controlled keystroke did the consumer's `setValue` and the field's `setDirty` / `setFilled` / `setValidityData` in one event handler, so React batched them into one render. At the head the field half runs in a layout effect after the consumer's render has committed. `validation.change` calls `commit(value, !validateOnChange)` (`useFieldValidation.ts:316-328`); when validating on change, `commit` always ends in `setValidityData(nextValidityData)` with a fresh object (`useFieldValidation.ts:302-313`), which schedules a second synchronous render. When not validating on change and the field is not invalid, `commit` returns early (`useFieldValidation.ts:169-172`) and nothing is scheduled.

**Measurement.** T1, using the same render-prop counter the PR's test uses, non-strict render: `onChange` mode 2 → 4 renders for two keystrokes; `onSubmit` mode 2 → 2. The same doubling applies in `onSubmit` mode after a submit attempt, and whenever an invalid field is revalidating, since those are the cases where `commit` publishes.

**Why it is a finding and not a nit.** `validityData` is part of `FieldRoot`'s memoised context value (`FieldRoot.tsx:150-182`), so the second commit re-renders every part that reads the field context, per keystroke. More importantly the test added to guard this exact property is titled "renders once per keystroke for controlled input changes" and says "The controlled echo must not schedule a second render per keystroke", and it passes only because it runs in the mode where validation is a no-op. A test whose stated invariant is false under the feature's primary configuration will mislead whoever next touches this path.

**Options.**

1. Keep the prop-driven model (it is simpler and it is what makes T8 correct) and make the suite tell the truth: retitle the test, drop or correct the line-78 comment, and add the `validationMode="onChange"` case asserting the real count so a future change to it is a visible decision.
2. If one commit per keystroke is a requirement for text inputs, keep user input on the event path and use the effect only for prop changes the handler did not just process. That is the `processedValueRef` design in the description. It costs a ref and a guard, and it reintroduces the question of what to do when the consumer rewrites the value, so it should not be chosen by default.

Recommendation: option 1, stated explicitly in the description.

## Finding 3 — filled latches across a control swap

**Where.** `FieldControl.tsx:99-103`.

**Base.**

```tsx
useIsoLayoutEffect(() => {
  const hasExternalValue = valueProp != null;
  if (validation.inputRef.current?.value || (hasExternalValue && valueProp !== '')) {
    setFilled(true);
  } else if (hasExternalValue && valueProp === '') {
    setFilled(false);
  }
}, [validation.inputRef, setFilled, valueProp]);
```

**Head.**

```tsx
useIsoLayoutEffect(() => {
  if (validation.inputRef.current?.value) {
    setFilled(true);
  }
}, [validation.inputRef, setFilled]);
```

The per-change half of the base effect is now `setFilled(serializedValue !== '')` in the sync callback, which is correct. The mount half lost its reset branch. `filled` is `React.useState(false)` on `FieldRoot` (`FieldRoot.tsx:52`) and is not cleared when a control unregisters, so the mount effect of the next control is the only thing that can bring it back down.

**Reproduction (T3).** `Field.Root` containing `<Control key="a" value="abc" />`; swap to `<Control key="b" value="" />`. Root has `data-filled` after the swap at the head; it does not at the merge-base.

**Remedy.**

```tsx
useIsoLayoutEffect(() => {
  setFilled(Boolean(validation.inputRef.current?.value));
}, [validation.inputRef, setFilled]);
```

No branch, and the effect states the truth in both directions. On a fresh root this is `setFilled(false)` on a `false` state, which React bails out of, so it adds no render. It also resets filled when the replacement control is an empty uncontrolled one, which the base did not do; that is the more consistent behaviour, but it is a small widening and should be covered by the added test. Verified with `alt`: T3 returns to the base result and no other probe moves.

**Not verified.** The head's mount effect now depends on the element behind `validation.inputRef` exposing `.value`. The base also treated a non-empty `value` prop as filled regardless of the element. A `render` target that does not forward the ref to an input-like element would therefore mount unfilled at the head where it mounted filled at the base. This was reasoned from the code and not probed.

## Finding 4 — description versus diff

| Description says | Head does | Evidence |
| --- | --- | --- |
| "`processedValueRef` records the value the change handler just handled, so the prop echo does not validate the same keystroke twice." | No such ref. Controlled changes return early from `onChange` (`FieldControl.tsx:144-146`). | Content search for `processedValueRef` under `packages/react/src` returns nothing. |
| "A control that unmounts or becomes disabled while focused no longer latches `data-focused`." | No focus code is touched; the only focus-related hunk is the unchanged `autoFocus` effect being moved below the new block. | T4: latched at base and at head, both cases. |
| "The sibling controls also call `clearErrors(name)` when their value changes. This PR does not." | `clearErrors(name)` at `FieldControl.tsx:110`. | T6: server error cleared by a programmatic change at the head, kept at the base. |

The third row is a user-visible change that the suite does not pin: "syncs state and validates when the controlled value changes programmatically" (`FieldControl.test.tsx:142-167`) renders no `Form`, so deleting line 110 would leave every test green.

## Finding 5 — the missing shared hook

Non-test `useValueChanged` call sites that sync field state, at the head:

`select/root/SelectRoot.tsx:243`, `radio-group/RadioGroup.tsx:176`, `checkbox/root/CheckboxRoot.tsx:210`, `checkbox-group/CheckboxGroup.tsx:119`, `switch/root/SwitchRoot.tsx:105`, `slider/root/SliderRoot.tsx:178`, `number-field/input/NumberFieldInput.tsx:101`, `otp-field/root/OTPFieldRoot.tsx:199`, `combobox/root/AriaCombobox.tsx:1055` and `:1070`, and now `field/control/FieldControl.tsx:105`.

Each restates the same sequence with a control-specific dirty predicate. The orderings differ: most mark dirty before `validation.change`; `SliderRoot` calls `validation.change` first and `setDirty` afterwards, even though `FieldControl` documents that `validation.change` reads `markedDirtyRef`. A sketch of the shared shape:

```ts
export function useFieldValueSync<Value>(
  value: Value,
  name: string | undefined,
  options: {
    isDirty: (value: Value, initialValue: unknown) => boolean;
    isFilled?: (value: Value) => boolean;
  },
) {
  const { setDirty, setFilled, validityData, validation } = useFieldRootContext();
  const { clearErrors } = useFormContext();

  useValueChanged(value, () => {
    clearErrors(name);
    // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
    setDirty(options.isDirty(value, validityData.initialValue));
    if (options.isFilled) {
      setFilled(options.isFilled(value));
    }
    validation.change(value);
  });
}
```

Controls with extra steps (Radio Group's ref re-point, OTP's pending focus, Number Field's revalidation block) would need either a trailing callback or to stay bespoke. This was not prototyped and is out of scope for a two-file fix; it is recorded because the description's premise is that `Field.Control` should look like its siblings, and the siblings do not yet have a single thing to look like.

## Non-blocking notes

- The `autoFocus` effect and `inputRef` declaration (`FieldControl.tsx:117-123`) are unchanged but appear in the diff as removed and re-added because the new block was inserted above them. Inserting the new code below would have kept the hunk to the lines that changed.
- `serializedValue = value == null ? undefined : String(value)` folds a controlled `null` into the "uncontrolled, ask the DOM" sentinel that `useRegisterFieldControl` uses, and the sync callback then needs an `undefined` guard. Probed (T9): setting a controlled value to `null` leaves the DOM text and filled unchanged at base, head and alt alike, so this is consistent with what React does and not a regression.
- Rewriting "avoids rerendering for uncontrolled input changes" to count `render`-prop calls is an improvement: the previous `RenderCountedControl` wrapper took no props and read no context, so its count could not move.
- File sizes at the head: `FieldControl.tsx` 207 lines, `FieldControl.test.tsx` 273 lines. No size concern.
