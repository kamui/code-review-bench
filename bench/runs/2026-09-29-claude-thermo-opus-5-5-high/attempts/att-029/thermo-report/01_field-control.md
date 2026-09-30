# 01 — `Field.Control` value-sync restructuring (detail)

Scope: `packages/react/src/field/control/FieldControl.tsx` at `review-head` (14d39e5d1), compared with `main` (30b8ea200).
Supporting code read: `internals/useValueChanged.ts`, `internals/field-register-control/useRegisterFieldControl.ts`,
`internals/field-register-control/useFieldControlRegistration.ts`, `field/root/FieldRoot.tsx` (`setDirty` / `markedDirtyRef`),
`field/root/useFieldValidation.ts`, and the sibling sync blocks in `number-field/input/NumberFieldInput.tsx`,
`otp-field/root/OTPFieldRoot.tsx`, and `checkbox/root/CheckboxRoot.tsx`.

## What the PR does, in one paragraph

Before the PR, `Field.Control` computed dirty, filled, error clearing, and validation synchronously in `onChange` for both
modes, and it had a mount-time layout effect that forced `filled` from `valueProp`. The PR registers a stringified
`serializedValue` with the field, drops the `valueProp` branch from the mount effect, adds a `useValueChanged(serializedValue, …)`
block for the controlled mode, returns early from `onChange` when controlled, and adds `details.isCanceled` to the uncontrolled
validation guard. The file grows from 179 to 207 lines, which is nowhere near the 1k-line bar. Adopting `useValueChanged` for the
controlled path is the right direction. It brings `Field.Control` in line with the sibling controls and removes the old
`valueProp`-specific mount branch. The findings below are about what the restructure left behind, not about the direction.

## Measurements and verification

Existing suite, run from the clone root:

```
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx
 ✓ |@base-ui/react| src/field/control/FieldControl.test.tsx (27 tests | 1 skipped) 194ms
      Tests  26 passed | 1 skipped (27)
```

The scratch harness lives in `clone-work/scratch/`: `vitest.config.mts`, `scratch.test.tsx`, `Base.tsx`, and `Variant.tsx`. It runs the
same probes against three implementations:

- `base`: `git show main:packages/react/src/field/control/FieldControl.tsx`, with relative imports rewritten to absolute clone paths.
- `pr`: the real `Field.Control` from the clone at `review-head`.
- `variant`: the PR file with the two code-judo moves from Findings 1 and 2 applied (diff below).

Command: `TZ=UTC <clone>/node_modules/.bin/vitest run --root clone-work/scratch --silent=false` (21/21 passed). Logged observations:

```
pr      numeric dirty { initial: false, mid: true, end: false }
pr      programmatic { filled: true, dirty: true, calls: [ 'external' ] }
pr      submit validate args [ [ 'string', '5' ] ]
pr      controlled prevented { validateCalls: 1, serverErrorVisible: true }
pr      uncontrolled cancel { validateCalls: 0, dirty: true, filled: true }
pr      null { dom: 'abc', filled: true }
pr      renders/keystroke onChange [ 2, 2, 2, 2 ]
variant numeric dirty { initial: false, mid: true, end: false }
variant programmatic { filled: true, dirty: true, calls: [ 'external' ] }
variant submit validate args [ [ 'string', '5' ] ]
variant controlled prevented { validateCalls: 1, serverErrorVisible: true }
variant uncontrolled cancel { validateCalls: 0, dirty: true, filled: true }
variant null { dom: 'abc', filled: true }
variant renders/keystroke onChange [ 2, 2, 2, 2 ]
base    numeric dirty { initial: false, mid: true, end: true }
base    programmatic { filled: true, dirty: false, calls: [] }
base    submit validate args [ [ 'number', 5 ] ]
base    controlled prevented { validateCalls: 0, serverErrorVisible: true }
base    uncontrolled cancel { validateCalls: 1, dirty: true, filled: true }
base    null { dom: 'abc', filled: true }
base    renders/keystroke onChange [ 1, 1, 1, 1 ]
```

The PR's two headline fixes show up in these probes: numeric dirty now clears, and programmatic changes now sync. The variant matches
the PR on every probe. The `controlled prevented` row is the behavioural asymmetry discussed in Finding 3.

---

## Finding 1 — The field-sync sequence is written twice, in two different orders (missed code-judo; verified)

`FieldControl.tsx:105-115` (the `useValueChanged` callback) and `FieldControl.tsx:148-156` (the uncontrolled `onChange` tail)
both do the same four operations: `setDirty(x !== (validityData.initialValue ?? ''))`, `setFilled(x !== '')`,
`clearErrors(name)`, and `validation.change(x)`. The PR body says the fix "gives each mode one owner instead of running two sync
paths at once". In the source, though, there are still two sync paths. They are now mutually exclusive, and each one is a
hand-written copy of the other. The copies already disagree on details. The controlled copy calls `clearErrors` before `setDirty`,
and the uncontrolled copy calls it after. Only the uncontrolled copy carries the ordering comment ("`validation.change` reads
`markedDirtyRef`, so update dirty before validating"). That comment states an invariant both copies depend on (see
`FieldRoot.tsx:69-77`, where `setDirty` writes `markedDirtyRef` synchronously), but the controlled copy has no reminder of it. The
`?? ''` fallback on `initialValue` and the `!== ''` filled rule are also duplicated. The next change to what "dirty" means for a
text input (trimming, for example, or a different baseline) has to be made twice. Missing one copy produces exactly the
mode-dependent drift this PR set out to fix.

Code-judo move: name the two concepts once and let each mode's owner call them. Dirty and filled are field *state* that always
follows the value. Error clearing and revalidation are the *reaction* that an intercepted change may skip:

```tsx
// `validation.change` reads `markedDirtyRef`, so callers must sync state before revalidating.
function syncFieldState(nextValue: string) {
  setDirty(nextValue !== (validityData.initialValue ?? ''));
  setFilled(nextValue !== '');
}

function revalidate(nextValue: string) {
  clearErrors(name);
  validation.change(nextValue);
}

useValueChanged(serializedValue, () => {
  if (serializedValue !== undefined) {
    syncFieldState(serializedValue);
    revalidate(serializedValue);
  }
});

// in onChange, after the controlled early-return:
syncFieldState(inputValue);
// Workaround for https://github.com/react/react/issues/9023
if (!event.nativeEvent.defaultPrevented && !details.isCanceled) {
  revalidate(inputValue);
}
```

With this shape, "one owner per mode" really holds: the owners differ only in *when* they fire, and the policy lives in one place. It
also puts the ordering invariant in a single comment next to the only code that has to respect it. Verification: the `variant`
column above applies exactly this change, and it behaves identically to the PR on every probe, including render counts.

## Finding 2 — Registering `serializedValue` is redundant; the DOM is already the registration's value source (missed code-judo; verified)

`FieldControl.tsx:85-97` introduces `serializedValue` and passes it as the `value` argument of `useRegisterFieldControl`, with the
comment "The DOM value is always a string, so dirty comparisons must serialize the controlled value." But `Field.Control` *also*
passes `getValueFromInput` (which reads `validation.inputRef.current?.value`) as the registration's `getValue` override. The
registration layer only uses `registration.value` in `getRegistrationValue`
(`useFieldControlRegistration.ts:48-50`: `registration.value === undefined ? getValueForForm() : registration.value`). It feeds
two consumers: the initial-value capture (`captureInitialValue`, line 97) and form-triggered validation (`validate`, line 61).
Form *values* come from `getValue`, meaning the DOM, in both modes. React writes a controlled input's `value` during the commit
mutation phase, before any layout effect runs, so the DOM already holds exactly `String(value)` when the registration effect
fires. The uncontrolled mode already relies on this path and registers `undefined`.

The PR is therefore fixing the `"5" !== 5` baseline bug by manually recreating a string that the registration would read from
the DOM anyway if the control simply stopped registering a second source. Registering `undefined` in both modes turns this into
a deletion. The `serializedValue` argument goes away, and so does the mode-specific shape of the registration. The
registration effect in `useRegisterFieldControl` also stops re-running on every controlled keystroke. Its deps include `value`,
so today every keystroke rebuilds the registration object and rewrites the form's `fields` Map entry via `refreshRegistration`.
This churn predates the PR, but the PR touched this exact line and could have removed it.
`serializedValue` would then only exist as the `useValueChanged` trigger, which is a single, obvious role.

Verification: the `variant` registers `undefined` and matches the PR on the numeric-dirty baseline probe (`initial: false,
mid: true, end: false`) and on the Form-submit probe (`validate` receives the string `'5'` in both). The PR changed that submit
argument from the number `5` on `main`, so the variant keeps the PR's new, consistent string contract.

Remedy: `useRegisterFieldControl(validation.inputRef, id, undefined, getValueFromInput, !disabled, nameProp);`, and delete the
serialization comment from the registration site. If reviewers prefer an explicit contract, a better option than a sentinel
`undefined` is to give `useRegisterFieldControl` a documented "read the value from `getValue`" mode. Either way, the control
should have a single value source for registration.

## Finding 3 — The two owners now disagree on what an intercepted change means (asymmetry; verified)

The old single path applied the `react#9023` guard (`event.nativeEvent.defaultPrevented`) to both modes. After the PR, the guard
and the new `details.isCanceled` check sit only in the uncontrolled tail (`FieldControl.tsx:152-156`). In controlled mode,
`onChange` returns at line 144 before reaching them, and `useValueChanged` then revalidates whatever value the consumer commits.
The scratch probe `controlled prevented` fires a prevented `input` event on a controlled control whose consumer echoes the value.
It records `validateCalls: 1` on the PR and `0` on `main`. On the PR, `clearErrors(name)` is also called on that path. The same
probe on an uncontrolled control still records 0 validations, which is what the existing test "does not clear errors or validate
when change is prevented" pins. That test only exercises the uncontrolled mode.

The same asymmetry applies to cancellation. In uncontrolled mode, `details.cancel()` skips validation but still updates dirty
and filled (`uncontrolled cancel { validateCalls: 0, dirty: true, filled: true }`). In controlled mode, cancellation has no
explicit meaning at all. It works only if the consumer happens not to call `setValue` after canceling. The PR body describes
this as "`details.cancel()` in `onValueChange` now stops the internal handling". The code actually implements "cancel skips
revalidation in uncontrolled mode", which is narrower.

This may well be the intended policy: a controlled consumer who commits a value has accepted it. But the policy is implicit. It is
spread across an early return and a two-flag guard, and it is documented only by a comment that now covers half the cases. With
Finding 1's split, the policy becomes explicit and easy to read: `syncFieldState` always runs, and `revalidate` is what
interception suppresses. Remedy: state the rule next to that guard. The rule is that interception suppresses revalidation only
for uncontrolled changes, and a controlled consumer expresses rejection by not committing. Then add a controlled-mode twin of
the "change is prevented" test so the chosen behaviour is pinned rather than incidental. If the intended rule is instead "a
prevented native event never revalidates", the controlled owner needs a way to see it, for example a ref set in `onChange` and
consumed in the `useValueChanged` callback. Choose that deliberately rather than leaving it as a side effect of the early return.

## Non-findings checked

- **File size**: 207 lines after the PR, well under the 1k bar.
- **`serializedValue === undefined` guard in `useValueChanged`** (`FieldControl.tsx:106`): it is only reachable on a controlled →
  `null`/`undefined` transition, which React itself treats as a switch to uncontrolled (the `null` probe shows the DOM keeps
  `'abc'`). The guard is defensive but harmless. It also narrows the type, so it earns its place, and Finding 1's `!== undefined`
  form reads more directly.
- **Render cost**: the variant has the same render counts as the PR (`[2, 2, 2, 2]` per keystroke in `onChange` mode), which
  matches the PR body's table. Base measured `[1, 1, 1, 1]`. The extra render is the known cost of moving to `useValueChanged`,
  and the PR body discloses it.
- **Mount-time `filled` effect**: dropping the `valueProp` branch is correct, because the DOM read covers the controlled mode too.
  The new test "does not set filled state on mount for an empty controlled value" pins the case the old `else if` handled.
