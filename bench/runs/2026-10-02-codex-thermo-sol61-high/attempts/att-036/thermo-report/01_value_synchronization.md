# Native value synchronization

## Evidence and judgment

The changed component retains sensible mode ownership: uncontrolled changes are observed through the DOM event, and accepted controlled values through `useValueChanged`. The hook is canonical and invokes its callback in a layout effect only when its observed value changes, without validating on mount. Keeping that ownership avoids validating consumer-rejected input and lets a programmatic change invalidate an older asynchronous validator. The scratch tests confirm both behaviors.

The boundary error is the meaning of the observed value. `FieldControl.tsx:85–86` treats `String(value)` as the DOM serialization. `FieldControl.tsx:90–97` registers that string as the baseline and imperative-validation value. `FieldControl.tsx:105–114` uses it for dirty, filled, and change validation. Meanwhile, `getValueFromInput` at line 88, the blur handler at lines 161–166, and the Enter handler at lines 169–172 read the native input. There are now competing representations of one native control.

The reproduced counterexample uses a normal controlled text input initialized to an empty string. Its callback accepts the edit by setting the controlled value to the edited text plus a newline. After the event, the controlled prop is `"a\n"`, but the native input's `value` is `"a"`. The head passes `"a\n"` to `validate`; the base passes `"a"`. This is a concrete regression, not an assertion based on an imagined browser rendering. It was observed in the permitted jsdom environment. The baseline/form getter split was partly present before the PR, but the new controlled effect extends that split into change validation instead of eliminating it.

The scratch reproducer is [field-boundaries.test.tsx](../field-review-tests/field-boundaries.test.tsx), in the test named `validation observes the accepted native input value after sanitization`. It first establishes that the input value is `"a"`, then compares the validator's argument to that value. The assertion fails on the head and passes against the base component. A validator can therefore reject text that the input no longer contains, and change validation can use a different value from blur or form projection. Other native input types also have value normalization rules; those additional cases were not executed and are not separate findings.

## Worked code-judo proposal

Use the architecture's existing DOM-value path for this native control. `useFieldControlRegistration.ts:48–49` already falls back to `getValueForForm()` when `registration.value` is undefined. Lines 41–42 already route that getter through the override supplied by FieldControl. This fallback is used by the uncontrolled mode today and captures the actual string value after the input ref is attached. It can also represent controlled native inputs without introducing another serializer or altering logical-value registrations for Number Field, Select, and other composite controls.

The following is a proposed local restructuring, not an applied or tested patch:

```tsx
// Register the native value source in both modes. Capture the baseline
// through the same getter used for form projection and explicit validation.
useRegisterFieldControl(
  validation.inputRef,
  id,
  undefined,
  getValueFromInput,
  !disabled,
  nameProp,
);

const synchronizeFieldValue = useStableCallback(
  (nextValue: string, runValidation: boolean) => {
    setDirty(nextValue !== (validityData.initialValue ?? ''));
    setFilled(nextValue !== '');
    if (runValidation) {
      clearErrors(name);
      validation.change(nextValue);
    }
  },
);

useValueChanged(valueProp, () => {
  if (!isControlled) {
    return;
  }
  const nextValue = getValueFromInput();
  if (nextValue !== undefined) {
    // Event acceptance policy is resolved at this boundary; see the
    // companion report before using an unconditional true here.
    synchronizeFieldValue(nextValue, true);
  }
});
```

The value prop becomes an observation signal, while the attached native control supplies the committed value. The uncontrolled handler calls the same state transition only after its acceptance policy is resolved. This deletes `serializedValue` and the assumption that JavaScript string coercion models native normalization. Numeric initial values still become strings through the DOM, so the PR's numeric dirty-baseline fix remains represented without special numeric or array branches. The existing `useControlled` warning machinery need not be removed to implement this change.

The explicit validation flag separates the historical native-prevention behavior from full cancellation: a prevented native event previously updated dirty and filled but did not clear errors or validate. Full cancellation should return before this transition. Do not make every control use DOM values; this proposal belongs to FieldControl because its public output and form projection are native strings. Do not teach the generic `useValueChanged` hook about inputs or cancellation.

## Verification and limits

The initial existing-suite command was:

```sh
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx
```

It passed with 26 tests passing and one skipped. The scratch config uses jsdom, aliases the two Base UI source packages, and links the installed React package dependencies. From the clone root, the scratch command was:

```sh
TZ=UTC ../clone/node_modules/.bin/vitest run --root ../clone-work/field-review-tests
```

That run produced three failing boundary checks and two passing ownership/async checks. After expanding failure assertions to capture both dirty/filled and validation/error outcomes, the suite was run once with `--reporter=verbose`, again producing three failures and two passes.

For the base comparison, only `main:packages/react/src/field/control/FieldControl.tsx` was copied into scratch space, and its relative imports were resolved to the clone's unchanged dependencies. `FIELD_CONTROL_REVISION=base` selects that component in the same harness. The command with `--testNamePattern 'controlled native prevention|validation observes'` passed both selected tests. This is a comparison of the two FieldControl implementations with identical helpers, not a claim that the entire base tree was checked out or tested.

The proposed restructuring was not executed. Its follow-up tests should retain numeric dirty-baseline coverage, add array baselines and native-sanitized controlled values, preserve mount behavior without initial validation, and retain the rejection/async checks. The browser-only autofocus/hydration test remained skipped because browsers are unavailable. No renderer inspection or external information was used.

## Measurements

`git diff --stat main...review-head` reports two changed files, 197 additions and 26 deletions. `wc -l` on the head and `git show main:<path> | wc -l` on the base report 179 to 207 lines for the implementation and 130 to 273 lines for the test file. The size increase is proportionate to the added tests and does not require decomposition. The opportunity for simplification is replacing competing value models with the getter that already owns the native form value.
