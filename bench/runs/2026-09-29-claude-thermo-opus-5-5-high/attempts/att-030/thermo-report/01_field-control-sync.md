# 01 — `Field.Control` value → field-state synchronization

Scope: `packages/react/src/field/control/FieldControl.tsx` (179 → 207 lines) and
`packages/react/src/field/control/FieldControl.test.tsx` (+149 / −6), range
`30b8ea200..14d39e5d1` (`git diff main...review-head`).

## What the diff does

The old component drove every field concern from the DOM `change` event plus a layout effect that
re-derived `filled` from `valueProp`. The PR:

1. serializes the controlled value (`serializedValue = value == null ? undefined : String(value)`,
   line 86) and registers that string with `useRegisterFieldControl` (lines 90–97), so the captured
   `initialValue` baseline is a string and the dirty comparison against the DOM string agrees;
2. reduces the mount effect to a one-time DOM read for `filled` (lines 99–103);
3. adds a `useValueChanged(serializedValue, …)` block that owns clear-errors / dirty / filled /
   validate for the controlled mode (lines 105–115);
4. makes `onChange` return early when controlled (lines 142–146) and adds `!details.isCanceled` to the
   pre-existing `defaultPrevented` gate on the uncontrolled path (line 153).

The direction is right: it brings `Field.Control` in line with Switch/Checkbox/RadioGroup/Select/…,
fixes the `"5" !== 5` dirty bug, and fixes programmatic-change staleness. The concerns below are about
how the two modes were wired, not about the goal.

## Verification performed

- Existing suite: `TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx`
  → 26 passed, 1 skipped (the SSR autofocus test is `skipIf(isJSDOM)`).
- Scratch probes (work dir `clone-work/scratch`, aliases pointing at the head tree) and the identical
  probes against the base tree extracted with `git -C clone archive main packages/react/src packages/utils/src`
  into `clone-work/scratch-base`. Probe file `probe.test.tsx`; outputs:

| Probe | Base (`30b8ea200`) | Head (`14d39e5d1`) |
| --- | --- | --- |
| A. Uncontrolled, `onValueChange={(v, d) => d.cancel()}`, `validationMode="onChange"`, type `a` | dirty=true, filled=true, validate calls=1 | dirty=true, filled=true, **validate calls=0**, DOM value `a` |
| B. Controlled (`value`/`setValue`), `<Form errors={STABLE}>`, capture listener calls `preventDefault()` on a cancelable `input`, type `a` | **validate calls=0, server error visible** | **validate calls=1, server error cleared** |
| C. Controlled, `onValueChange` calls `details.cancel()` then `setValue(v)` anyway | validate calls=1 | validate calls=1 |

(A first run of probe B used an inline `errors={{…}}` literal; `Form` re-applies a new `errors` object
on every parent render through its own `useValueChanged`, which masked the result. The table uses a
hoisted constant.)

## Finding 1 — Two owners, two diverging copies of the same sync sequence (structural / code-judo)

The PR's thesis is "one owner per mode", but what landed is two copies of one algorithm, each
hand-inlined at its trigger and already drifting:

```ts
// lines 105–115 — controlled owner
useValueChanged(serializedValue, () => {
  if (serializedValue === undefined) {
    return;
  }
  clearErrors(name);
  setDirty(serializedValue !== (validityData.initialValue ?? ''));
  setFilled(serializedValue !== '');
  validation.change(serializedValue);
});

// lines 148–156 — uncontrolled owner (inside onChange, after `if (isControlled) return;`)
// `validation.change` reads `markedDirtyRef`, so update dirty before validating.
setDirty(inputValue !== (validityData.initialValue ?? ''));
setFilled(inputValue !== '');
if (!event.nativeEvent.defaultPrevented && !details.isCanceled) {
  clearErrors(name);
  validation.change(inputValue);
}
```

Drift already visible in the first revision:

- **Order.** Controlled calls `clearErrors` before `setDirty`; uncontrolled calls `setDirty` first. The
  ordering constraint ("`validation.change` reads `markedDirtyRef`, so update dirty before validating")
  is documented on only one copy. The controlled copy happens to satisfy it, but nothing ties it to
  the rule.
- **Gating.** The uncontrolled copy gates half the sequence on `defaultPrevented`/`isCanceled`; the
  controlled copy has no gate at all (see Findings 2 and 3 for what that means behaviorally).
- **Mode detection in two places, in two different vocabularies.** The controlled owner detects
  "uncontrolled" as `serializedValue === undefined`; the uncontrolled owner detects "controlled" as
  `isControlled`. These are not quite the same predicate (`value={null}` is `isControlled === true`
  but `serializedValue === undefined`, so a null controlled value is owned by *neither* path). A
  reader has to prove the two guards partition the modes; the code does not make it obvious.

### Worked code-judo proposal

Make the algorithm a single named function and keep the mode decision exactly once at each trigger.
Behavior for the non-canceled, non-prevented path is unchanged:

```ts
// The DOM value is always a string, so compare and validate the string form of the value.
const domValue = value == null ? undefined : String(value);

const syncFieldValue = useStableCallback((nextValue: string) => {
  // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
  setDirty(nextValue !== (validityData.initialValue ?? ''));
  setFilled(nextValue !== '');
  clearErrors(name);
  validation.change(nextValue);
});

// Controlled: the `value` prop is the source of truth.
useValueChanged(domValue, () => {
  if (domValue !== undefined) {
    syncFieldValue(domValue);
  }
});

// …
onChange(event) {
  const inputValue = event.currentTarget.value;
  const details = createChangeEventDetails(REASONS.none, event.nativeEvent);
  onValueChange?.(inputValue, details);

  // Controlled: sync happens when the consumer commits the value (above).
  // Workaround for https://github.com/facebook/react/issues/9023
  if (isControlled || event.nativeEvent.defaultPrevented || details.isCanceled) {
    return;
  }
  syncFieldValue(inputValue);
},
```

What this deletes: the second copy of the four-step sequence, the second (and only-half-applied)
gate, and the order drift. What remains is two triggers and one algorithm, which is the actual shape
of the problem. Note that moving the gate to the top of `onChange` changes the uncontrolled
canceled/prevented case to also skip `setDirty`/`setFilled`; that is intentional and is the subject
of Finding 2 — the author should pick the semantics once, and the helper makes whichever choice is
picked apply uniformly. If the author wants to keep today's split (dirty/filled always, validation
gated), the helper can take the gate as a parameter, but that split should then be justified in a
comment and pinned by a test.

Rename note: `serializedValue` reads as "the output of `internals/serializeValue.ts`", which is the
repository's canonical serializer and has different semantics (`JSON.stringify` for arrays, `''` for
nullish). A maintainer "reusing the canonical helper" there would silently break dirty tracking for
array values (`'["a","b"]'` vs DOM `'a,b'`). A name like `domValue` states the invariant that
actually matters.

### Fuller judo considered and rejected

The siblings the PR cites (Number Field, OTP Field, Combobox, Checkbox) have *one* owner for *both*
modes, because they keep the uncontrolled value in React state and let `useValueChanged` see every
change. `Field.Control` could do the same by using the setter returned by `useControlled` in
`onChange`, which would delete the uncontrolled branch entirely. I do not recommend it: the existing
test `avoids rerendering for uncontrolled input changes` pins a per-keystroke render budget for the
uncontrolled input that state tracking would break, and the mount-time DOM read (browser-restored
values) would have to seed that state. The single-helper version above gets most of the
simplification without that cost. It is worth stating in the PR body that the claimed "same shape as
its siblings" is really "same shape in controlled mode only".

## Finding 2 — `details.cancel()` in uncontrolled mode half-applies the change (atomicity)

Line 153 adds `!details.isCanceled` to the gate, but the gate sits *after* `setDirty`/`setFilled`
(lines 149–150). Probe A at head: after a canceled change the root reports `data-dirty` and
`data-filled` for the new DOM value `a`, while `validate` was never called and form errors were not
cleared. The field now shows dirty/filled state for one value and validity for another. That is the
same class of staleness the PR body sets out to fix ("updated the input text but not filled, dirty,
or validity"), just reached through a different door. The PR body describes the change as
"`details.cancel()` in `onValueChange` now stops the internal handling", which is only half true.

In the controlled mode, cancel has no effect of its own at all (probe C): the sync is driven purely
by whether the consumer commits the value. That is defensible and matches Number Field/OTP Field,
but it means the new `isCanceled` check is live only in the uncontrolled branch, where it is
partial.

Remedy: decide what cancel means for an uncontrolled `Field.Control`, whose DOM value cannot be
reverted, and apply it to the whole sync. The closest match to the siblings (where a canceled change
leaves internal state untouched) is to gate the entire `syncFieldValue` call as in the Finding 1
proposal. Pin the choice with a test that asserts dirty, filled and validation together, not only
`validate` (the new test `does not validate when the change is canceled` asserts only `validate`).

## Finding 3 — The React #9023 `defaultPrevented` guard silently stops applying to controlled inputs (verified behavior change)

At base, `onChange` gated `clearErrors(name)` and `validation.change` on
`!event.nativeEvent.defaultPrevented` for both modes. At head, controlled `onChange` returns before
the guard (lines 142–146) and the controlled sync runs from `useValueChanged` without any guard.
Probe B: a controlled `Field.Control` inside `<Form errors={{ message: 'Server error' }}>` receives a
prevented, cancelable `input` event; base leaves the server error visible and does not validate;
head clears the server error and validates once. The only test pinning the guard,
`does not clear errors or validate when change is prevented`, uses an uncontrolled control, so the
suite does not notice.

This may be an acceptable consequence of "the `value` prop is the source of truth", but the PR body
does not mention it, the workaround comment now reads as if it still covers the component, and the
test name promises a contract that is mode-dependent. Remedy: either state the new contract
explicitly (comment on the guard: "uncontrolled only; controlled inputs sync from `value`") and add a
controlled twin of the test pinning the new behavior, or carry the prevention across to the
controlled sync (for example a one-shot ref set in `onChange` and consumed in the `useValueChanged`
callback, which is the pattern Number Field already uses with `blockRevalidationRef`). The first is
simpler and is what I would pick unless the #9023 case is known to occur with controlled inputs.

## Non-findings checked

- File size: 179 → 207 lines, far from any threshold.
- The render-cost trade in `onChange` validation mode (one extra render per keystroke) is stated and
  matches Number Field; the new test `renders once per keystroke for controlled input changes`
  covers only the default `onSubmit` mode, which is consistent with the PR's table.
- Registration now carries the string form of the controlled value. The form's `getValue` is still
  overridden by `getValueFromInput` (DOM read), so the serialized registration value only feeds the
  `initialValue` capture in `useFieldControlRegistration.ts`; no other consumer changes.
- `String(value)` agrees with how React writes a non-string `value` into the DOM (numbers, arrays via
  `toString`), so the dirty baseline and the DOM comparison agree for the cases the PR targets.
- `value={null}`: React treats the input as uncontrolled and keeps the typed DOM value, so the
  `filled`/`dirty` flags remaining set after a switch to `null` is not stale (probed, then dropped as
  a finding).
