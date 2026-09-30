# 01 — `Field.Control` value-sync restructuring

Scope: `packages/react/src/field/control/FieldControl.tsx` (179 → 207 lines), range
`30b8ea2004fa999bed151204208676c6c0a9d261..14d39e5d1ad6b7aca2fb067415dba09c6bea219b`
(`git diff main...review-head`). `Input` (`packages/react/src/input/Input.tsx`) renders
`Field.Control`, so everything below also applies to `<Input>`.

## What the diff does

The old component ran field bookkeeping from two places at once: a layout effect keyed on
`valueProp` that set `filled`, and the `onChange` handler that set `dirty`/`filled`, cleared
errors and called `validation.change` for both modes. The new component:

1. Serializes the controlled value (`serializedValue = value == null ? undefined : String(value)`,
   line 86) and registers that string with `useRegisterFieldControl` (lines 90–97).
2. Keeps a mount-only layout effect that sets `filled` from the DOM value (lines 99–103).
3. Adds `useValueChanged(serializedValue, …)` (lines 105–115) that clears errors, sets dirty and
   filled, and calls `validation.change` when the controlled value changes.
4. Makes `onChange` return early when controlled (lines 142–146) and gates the uncontrolled
   validation on `!details.isCanceled` in addition to `!defaultPrevented` (line 153).

The direction is right. Moving controlled sync onto `useValueChanged` puts `Field.Control` in the
same shape as `SwitchRoot`, `CheckboxRoot`, `RadioGroup`, `SelectRoot`, `OTPFieldRoot`,
`NumberFieldInput` and `SliderRoot` (all verified with `rg "useValueChanged\(" packages/react/src`),
and deleting the `valueProp`-keyed filled effect removes one of the two competing sync paths. The
removal of that effect's `valueProp === ''` branch is covered by the new owner: on mount the DOM
read handles an empty controlled value, and later changes go through `useValueChanged`.

## Finding 1.1 — The field-sync block is now written twice, with drifting order and gating

**Where:** `FieldControl.tsx:105-115` (controlled owner) and `FieldControl.tsx:148-156`
(uncontrolled owner).

**Evidence.** The two owners perform the same four operations against the same field API:

```tsx
// controlled owner, lines 105-115
useValueChanged(serializedValue, () => {
  if (serializedValue === undefined) {
    return;
  }

  clearErrors(name);
  setDirty(serializedValue !== (validityData.initialValue ?? ''));
  setFilled(serializedValue !== '');

  validation.change(serializedValue);
});

// uncontrolled owner, lines 148-156
setDirty(inputValue !== (validityData.initialValue ?? ''));
setFilled(inputValue !== '');

// Workaround for https://github.com/react/react/issues/9023
if (!event.nativeEvent.defaultPrevented && !details.isCanceled) {
  clearErrors(name);
  validation.change(inputValue);
}
```

The dirty expression `x !== (validityData.initialValue ?? '')` and the filled expression
`x !== ''` are copied verbatim. The statement order differs (`clearErrors` comes first in one
copy and after `setDirty`/`setFilled` in the other), and so do the gates (the controlled copy has
none). The file's own comment explains that `validation.change` reads `markedDirtyRef`, so dirty
must be set first. That ordering rule now lives in two places, and only one of them carries the
comment.

**Why it matters.** The PR's premise is "one owner per mode." What it actually ships is one
*trigger* per mode, with the *policy* copied into each trigger. The next person to change how
dirty is computed (trimming, a custom equality, a `number` baseline) has to find and update both
copies. The drift that already exists (order, gating) is what the next bug will look like. The
mode split is also expressed as two unrelated ad-hoc guards: an early `return` placed mid-handler
(`if (isControlled) return;`) and a `serializedValue === undefined` sentinel inside the effect. A
reader has to put these together to see the ownership model.

**Code-judo proposal.** Name the policy once and keep the two triggers as thin call sites:

```tsx
// The DOM value is always a string, so dirty comparisons must serialize the controlled value.
const serializedValue = value == null ? undefined : String(value);

// `validation.change` reads `markedDirtyRef`, so update dirty before validating.
function syncFieldValue(nextValue: string, revalidate: boolean) {
  setDirty(nextValue !== (validityData.initialValue ?? ''));
  setFilled(nextValue !== '');

  if (revalidate) {
    clearErrors(name);
    validation.change(nextValue);
  }
}

useValueChanged(serializedValue, () => {
  if (serializedValue !== undefined) {
    syncFieldValue(serializedValue, true);
  }
});

// …
onChange(event) {
  const inputValue = event.currentTarget.value;
  const details = createChangeEventDetails(REASONS.none, event.nativeEvent);
  onValueChange?.(inputValue, details);

  // Controlled values sync from the `value` prop instead, so that a value the consumer
  // rejects or rewrites never reaches the field state.
  if (!isControlled) {
    // Workaround for https://github.com/react/react/issues/9023
    syncFieldValue(inputValue, !event.nativeEvent.defaultPrevented && !details.isCanceled);
  }
},
```

This preserves behavior. The state setters and `clearErrors` are batched in both the event
handler and the layout effect, so moving `clearErrors` after `setDirty`/`setFilled` in the
controlled path changes nothing observable. The result has one dirty/filled definition and one
home for the ordering comment. The gating difference also becomes a visible argument
(`revalidate`) instead of a structural difference between two blocks, which is what makes
Finding 1.2 below easy to see and decide.

**Verification status:** confirmed by reading. The behavior-preservation claim for the proposal
rests on React's automatic batching of the setters; it was not run as a patch, because the clone
is read-only.

## Finding 1.2 — The react#9023 `defaultPrevented` gate (and `cancel()`) now applies only to uncontrolled mode

**Where:** `FieldControl.tsx:142-156` and `FieldControl.tsx:105-115`.

**Evidence.** Before the PR, `onChange` gated `clearErrors` + `validation.change` on
`!event.nativeEvent.defaultPrevented` in *both* modes. After the PR, the gate sits after the
`if (isControlled) return;` early exit, so it only guards uncontrolled mode. The controlled owner
(`useValueChanged`) cannot see the event, so it clears errors and validates unconditionally
whenever the consumer commits the value. The existing test `does not clear errors or validate when
change is prevented` (`FieldControl.test.tsx`) only exercises an uncontrolled control, so this
change goes through CI without a signal.

A scratch probe (outside the clone; see `02_tests-and-verification.md` for the harness) ran the
existing prevented-input scenario with a controlled control
(`<Field.Control value={v} onValueChange={setV} />`, `validationMode="onChange"`, a capture-phase
`input` listener calling `preventDefault()`, then `fireEvent.input(…, { cancelable: true })`):

| Snapshot | `validate` calls after the prevented input |
| --- | --- |
| base (`main`) | 0 |
| head (`review-head`) | 1 |

**Why it matters.** This is a behavior change that the PR body does not mention: the
"two smaller fixes" section talks about `cancel()` starting to work, not about the `defaultPrevented`
workaround stopping for controlled inputs. It is also exactly the kind of asymmetry the
one-owner-per-mode restructuring produces when the policy is copied instead of shared. One owner
received the gate and the other did not. `details.cancel()` follows the same pattern. In
controlled mode it only "stops internal handling" if the consumer also declines to commit the
value, because the controlled owner never sees the cancellation.

**Remedy.** Decide the controlled-mode semantics explicitly and pin them with a test.

- If the #9023 gate is meant to guard only the case where the DOM changed without an intentional
  commit, then controlled mode does not need it, because the consumer's `setState` *is* the
  commit. In that case, say so next to the gate (for example, "controlled values are gated by the
  consumer; see the early return above") and add a controlled variant of the prevented-input test
  that asserts the new behavior.
- If the gate must still hold for controlled inputs, avoid adding a one-shot "skip next sync" ref
  (the `blockRevalidationRef` pattern in `NumberFieldInput`). That is the kind of cross-owner flag
  this restructuring was meant to remove. Instead, reconsider whether controlled `onChange` should
  forward `revalidate: false` into the shared helper from Finding 1.1 for that event. Either way,
  the helper makes the choice a single argument rather than a structural fork.

**Verification status:** confirmed by execution (base vs. head scratch probe).

## Finding 1.3 — Serializing the registration value silently changes the `validate` contract on submit

**Where:** `FieldControl.tsx:85-97`; consumer at
`packages/react/src/internals/field-register-control/useFieldControlRegistration.ts:48-62`
(`getRegistrationValue` returns `registration.value` when it is defined, and `validate()` commits
it).

**Evidence.** The PR body describes serialization as a dirty-check fix ("the control now
registers the serialized value, so the baseline and the comparison agree"). But
`registration.value` is also what `validate()` commits when the form validates the field on
submit, and what `captureInitialValue` stores as `validityData.initialValue`. A scratch probe with
`<Form><Field.Root name="n" validate={validate}><Field.Control value={5} …/></Field.Root></Form>`
and a submit click recorded the first argument passed to `validate`:

| Snapshot | `validate` argument on submit |
| --- | --- |
| base | `number` `5` |
| head | `string` `"5"` |

**Why it matters.** The new behavior is arguably the *more* coherent one, because the `onChange`
and `onBlur` paths already pass the DOM string (`validation.commit(event.currentTarget.value)`).
But the field's canonical value type has been changed as a side effect of a dirty-state fix, and
nothing in the code or the PR records that. `validate` is typed `(value: unknown, …)`, so the type
system cannot catch a consumer who wrote `value > 10` against a numeric controlled value.

**Remedy.** Treat the serialized string as the deliberate boundary it now is. Rename
`serializedValue` to something like `fieldValue` and widen the comment on line 85 to say it is the
value the field (dirty baseline, filled, validation) sees in every path. Then add a test that pins
the submit-time `validate` argument for a non-string controlled value. Also mention it in the
release notes, since `Input` inherits it.

**Verification status:** confirmed by execution (base vs. head scratch probe).

## Non-findings checked

- **File size.** 179 → 207 lines, far from the 1k threshold.
- **`value={null}` edge.** For both base and head, a controlled value going from `'x'` to `null`
  leaves `data-filled` set, and the DOM keeps `'x'` (React treats `null` as uncontrolled). The
  `serializedValue === undefined` guard preserves base parity, so this is not a regression.
- **Type-only change without a string change.** `5 → '5'` does not re-run validation at head
  (0 calls, the same as base), because the effect compares serialized strings. That is the
  intended outcome.
- **Render cost.** In `onChange` validation mode, a controlled input re-renders twice per
  keystroke at head versus once at base (probe deltas `2 2 2` vs `1 1 1`). This matches the PR
  body's own table and the sibling controls, so it is disclosed and accepted.
