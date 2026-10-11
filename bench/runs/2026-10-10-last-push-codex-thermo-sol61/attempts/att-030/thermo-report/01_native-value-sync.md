# Native value synchronization

## Evidence and design assessment

The production file grows from 179 to 207 lines. The change replaces event-driven controlled state updates with a serialized prop observer. At FieldControl.tsx:83–86, the native value is modeled as `String(value)`; lines 90–97 register that value; lines 105–115 apply it to dirty, filled, errors, and change validation. At lines 142–145, controlled DOM events return after the consumer callback, regardless of whether the native text actually changed independently of the prop.

The general `useValueChanged` hook at internals/useValueChanged.ts:6–16 initializes its comparison ref from the mount value and calls its callback only when that argument changes by strict inequality. That behavior is appropriate for sibling controls with logical checked/selected state. A native number input also owns textual representation, and a native text input sanitizes its value. Neither property is represented by strict comparison of this serialized prop.

The useful code-judo move is to make the native input's committed text the single value model. FieldControl already has `getValueFromInput` at line 88. The registration helper already knows how to obtain the baseline and validation value through that getter when its explicit value is undefined. Building an independent string serialization boundary misses that existing capability and leaves event validation, imperative validation, and Form submission with different sources of truth.

## Finding: unchanged props can hide changed native text

The early controlled return at FieldControl.tsx:142–145 is a behavioral regression. The scratch test mounts a number input with numeric initial value 5 and maps the callback string back through `Number`. It changes the DOM text to `"05"`. React leaves that spelling in the native element, while React state and `serializedValue` remain 5 and `"5"`, respectively. No prop-effect transition fires.

On the head, the DOM-value assertion for `"05"` passes, but dirty is false, the validator call count is zero, and its last value is undefined. The expected native-text behavior is dirty=true and one validation call with `"05"`. The merge-base FieldControl passes all three expectations in a targeted scratch substitution. This is not a hypothetical browser-only concern; it reproduces with the installed jsdom and React.

The installed React implementation also explains the observation. `packages/react/node_modules/react-dom/cjs/react-dom-client.development.js:1644–1670` contains `updateInput`; for a number input it uses a loose comparison, `element.value != value`, before replacing the native string. `"05"` and 5 compare numerically equal. An initial hypothesis that empty native text would be retained for numeric zero was rejected: React explicitly resets that case to `"0"`. That hypothesis is not a finding and is absent from the final scratch fixture.

Do not fix this by immediately validating every attempted controlled string. The existing new design deliberately waits for the consumer's accepted value. The positive scratch tests establish that a rejected edit restores empty native text without dirty state or validation, and a lowercase edit rewritten to uppercase validates only the uppercase value once. A remedy must observe the settled native value after React has applied or restored the controlled value, including the no-prop-change case.

## Finding: serialization is not native normalization

The new representation boundary at FieldControl.tsx:83–94 is inaccurate for native text normalization. The scratch test starts with an empty controlled text input in a named Form field and programmatically sets its prop to a newline. The native `.value` is still empty. Form projection, through `getValueFromInput`, also returns empty text. The new prop observer instead uses a newline string for dirty, filled, and the validator's first argument.

The scratch test confirms native value `""` and `formValues` equal to `{ message: '' }`, then fails three soft assertions: the validator's first argument is `"\n"` instead of `""`, filled is true instead of false, and dirty is true instead of false. A custom validator can therefore see different values for its own field depending on which argument it reads. This is an introduced synchronization inconsistency. Some baseline/registration mismatch for normalized controlled props existed previously; this report does not claim that all native normalization problems first appeared in this PR.

The same explicit registration value is used for imperative validation. At internals/field-register-control/useFieldControlRegistration.ts:35–49, Form projection prefers the getter, whereas `getRegistrationValue` uses the explicit registration value unless it is undefined. Lines 52–61 feed that latter value to `commit`. Baseline capture at lines 92–101 uses the same registration-value function. Therefore replacing a number with a string solves only the simplest numeric baseline case, and does not establish agreement with the native value.

## Worked code-judo proposal

Use the native registration path that already exists. The following is a design sketch, not an applied patch:

```tsx
const getValueFromInput = useStableCallback(
  () => validation.inputRef.current?.value,
);

useRegisterFieldControl(
  validation.inputRef,
  id,
  undefined,
  getValueFromInput,
  !disabled,
  nameProp,
);
```

Undefined here has an existing, explicit meaning: use the getter for baseline capture and imperative validation. It removes `String(value)` as the authoritative field representation, without adding another serializer or broadening an unknown-valued boundary. The registration baseline must still be captured once by Field.Root, as it is today; do not recapture it on each prop change or remount. The getter must remain live so form submission sees the latest committed value without requiring registration churn for every keystroke.

Next centralize dirty/filled/error/validation application, as worked in the event-policy report. Reconcile native text in two circumstances: after an external prop commit, and after a user attempt has settled even if the prop did not change. Both should read the live native text and use a shared last-applied native-string comparison. For mount, initialize the baseline and filled state without starting change validation. For an actual native-string transition, update dirty before validation to preserve `markedDirtyRef` ordering.

A prop observer can trigger reconciliation, but its callback should read `.value` rather than its argument. It cannot be the only trigger: the `"05"` case proves that native edits also need reconciliation. Observe a controlled user attempt after the consumer update and React's restoration, rather than assuming that the text present during `onValueChange` is already accepted. That requires a small native-input adapter with explicit settlement semantics; the sketch above alone does not fix the unchanged-prop case.

Keep this adapter local to FieldControl or a dedicated native-control hook. Do not change the generic `useValueChanged` equality contract for all controls, and do not scatter number/date/text-specific checks into the FieldRoot validator. Reading the actual native value removes whole categories of sanitizer rules from library code. The event-policy boundary should distinguish canceled input attempts, validation-prevented attempts, and external prop updates; a retained event marker must be consumed rather than accidentally suppressing a later reset.

The remedy should preserve steady-state render counts. It should not introduce React state merely to mirror the uncontrolled input text, and it must deduplicate a user edit followed by a controlled echo. The existing render-count test is a useful constraint, alongside behavioral tests; avoiding an extra render is not sufficient evidence of correct native-value observation.

## Verification

All commands ran offline and completed well within five minutes. From the clone root:

```sh
git diff main...review-head -- packages/react/src/field/control/FieldControl.tsx packages/react/src/field/control/FieldControl.test.tsx
wc -l packages/react/src/field/control/FieldControl.tsx packages/react/src/field/control/FieldControl.test.tsx
git show main:packages/react/src/field/control/FieldControl.tsx | wc -l
git show main:packages/react/src/field/control/FieldControl.test.tsx | wc -l
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/root/FieldRoot.test.tsx packages/react/src/input/Input.test.tsx packages/react/src/form/Form.test.tsx
git diff --check main...review-head
```

The changed-file measurements are +48/−20 for production and +149/−6 for tests, producing 207 and 273 lines. FieldControl's existing suite passed 26 tests, skipping one; the related selection passed 139, skipping three. Diff whitespace checking passed. There is no 1,000-line crossing or independent file-size blocker.

Scratch fixtures live in `../thermo-scratch` relative to this report directory. `vitest.config.mts` is a plain object with jsdom and source aliases, and its node_modules link points at the clone's installed React package dependencies. From the clone root, the full final scenario set was run using:

```sh
TZ=UTC ./node_modules/.bin/vitest run --root ../clone-work/thermo-scratch --reporter=verbose
```

That full selection passed the rejected-value, rewritten-value, and async-reset controls, and failed five contract scenarios. The initial exploratory run used an incorrect zero/empty expectation; the fixture was corrected to the reproduced equivalent-number-spelling case before this verbose selection. Soft assertions were added to expose all state discrepancies without stopping at the first one. Subsequent changes only strengthened the prevention fixture; those targeted runs are recorded in the event-policy report.

For the merge-base comparison, `git show main:packages/react/src/field/control/FieldControl.tsx` was written to the scratch directory as `FieldControl.base.tsx`. Relative imports were rewritten to absolute paths into the clone. `vitest.base.config.mts` aliases the field entry point's `./control/FieldControl` import to that scratch source. FieldControl is the only changed production file in this PR, so its production dependency source remains the same for this comparison. No checkout file was replaced or modified.

```sh
TZ=UTC ./node_modules/.bin/vitest run --root ../clone-work/thermo-scratch --config ../clone-work/thermo-scratch/vitest.base.config.mts --testNamePattern='native prevention|preventBaseUIHandler|number spelling' --reporter=verbose
```

All three selected merge-base scenarios passed; five other scratch cases were skipped by selection. The native prevention fixture was later strengthened to use stable external errors and assert invalid state on the head; that strengthened variant was not rerun against the base. The original comparison already establishes that validation prevention regressed.

No proposed remedy was implemented or executed. Browser output, other number-edit spellings, all other native input sanitizers, SSR autofocus, and transition/concurrency scheduling are not settled by these tests. The findings rely on observed cases and the inspected implementation, not assumptions about those unexecuted combinations.
