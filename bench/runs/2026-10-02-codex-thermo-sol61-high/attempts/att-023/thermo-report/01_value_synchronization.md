# Value synchronization and registration

## Scope and measurements

The production change is confined to `packages/react/src/field/control/FieldControl.tsx`, +48/-20. Its test file is +149/-6. `git diff main...review-head --numstat` produced those counts. `wc -l` measured 207 production lines and 273 test lines at the head; `git show main:<path> | wc -l` measured 179 and 130 at the merge-base. There is no 1,000-line crossing or giant-file blocker.

The previous implementation updated filled state in a value-dependent layout effect and did dirty/error/validation work in the DOM change handler. The head removes the old value-dependent filled effect, keeps one mount-time DOM read, and adds a controlled observer. Lines 142–145 leave controlled handling to that observer. This is a real ownership improvement: rejected values do not flow directly from the event into validation, and programmatic prop changes now have a synchronization path.

The head retains two copies of the field-update sequence, at lines 110–114 and 148–155. The copies have different ordering of error clearing and different permission gates. This duplication is small in line count, but important because its boundary differences cause the event defects documented in the other detail report. A single ordered operation would earn its abstraction by enforcing dirty-before-validation and a common committed value, rather than merely wrapping a setter.

## Finding: String conversion is not the native control value boundary

`FieldControl.tsx:85–86` converts the controlled value with `String`. Lines 90–97 use that value for registration, and lines 105–114 use it for dirty, filled, and validation. Meanwhile the existing getter at line 88 reads `validation.inputRef.current?.value`, and Form value projection continues to use that getter. Native inputs sanitize their values; their values are strings, but that does not imply they equal string conversion of the prop.

The concrete reproduction mounts a controlled text input with `value='hello'` in a named Field under Form, then changes the prop to a string containing one newline. The actual native input value becomes empty. The validator receives a newline as its first argument and `{ message: '' }` as its second argument. Filled state becomes true because the raw serialized prop is nonempty. A custom validator that decides emptiness from its first argument now sees something different from the submitted field value and actual input.

This reproduction requires no number-input typing or browser-specific badInput behavior. It runs in the permitted jsdom environment. The failing contract test reaches the validator-argument mismatch after successfully asserting that the DOM value is empty and the Form value is empty. The separate passing snapshot test verifies filled state is also true.

The raw registration baseline could already disagree with sanitized input values before this PR. That pre-existing baseline problem is not claimed as a new standalone regression. The newly introduced controlled observer now drives validation and filled state from the same unnormalized representation; that is the reviewed defect. The numeric dirty-state repair is useful for ordinary inputs but does not establish the broader invariant claimed by the serialization comment.

## Worked code-judo proposal

Use the existing native getter as the value-domain boundary. `useFieldControlRegistration.ts:48–50` already selects the getter when the registration's value is undefined; lines 91–101 capture its result once as the initial baseline. `getValueForForm`, at lines 35–45, uses that same getter for Form projection. Consequently Field.Control can use the same existing registration mode for controlled and uncontrolled native inputs:

```tsx
useRegisterFieldControl(
  validation.inputRef,
  id,
  undefined,
  getValueFromInput,
  !disabled,
  nameProp,
);
```

The control's value is then live through the getter, including for actionsRef validation, which resolves registration through `getRegistrationValue`. The registry does not need raw prop data to submit a native input. Do not change the generic registration helper or logical-value siblings such as Number Field and Checkbox.

One local operation can apply the field projection in the required order:

```tsx
const applyFieldValue = useStableCallback(
  (nextValue: string, validateChange: boolean) => {
    setDirty(nextValue !== (validityData.initialValue ?? ''));
    setFilled(nextValue !== '');
    if (validateChange) {
      clearErrors(name);
      validation.change(nextValue);
    }
  },
);
```

The controlled observer can keep `serializedValue` as a change signal, read `getValueFromInput()` after React has committed the input, and apply the resulting string. The uncontrolled handler already receives a native string from `event.currentTarget.value`. Canceled events return before invoking the operation. `validateChange` represents the existing native-prevention permission, not a value-mode switch. The event detail report explains why that permission cannot simply be assumed true for every controlled echo.

This deletes separate raw-prop and DOM value semantics without implementing a sanitizer, branching on type, or widening shared contracts. The mount-time DOM read remains because `useValueChanged` intentionally does not run for the initial value. Preserve the uncontrolled performance path; adding local value state solely to unify effect timing would introduce extra renders without solving the boundary defect.

This is a proposed decomposition, not a tested replacement. Retain coverage for numeric dirty restoration, initial prefill, empty controlled mount, programmatic clear, rejected/rewritten controlled input, and one validation per accepted transition. Add a newline-only input case and a rendered textarea normalization case if broader coverage is desired. Validate that registration's mount baseline is captured before subsequent dirty comparisons, including Strict Mode and field remounts.

## Commands and verification

From the clone root:

```sh
git diff main...review-head -- packages/react/src/field/control/FieldControl.tsx
git diff main...review-head -- packages/react/src/field/control/FieldControl.test.tsx
git diff main...review-head --numstat
wc -l packages/react/src/field/control/FieldControl.tsx packages/react/src/field/control/FieldControl.test.tsx
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/root/FieldRoot.test.tsx packages/react/src/field/validity/FieldValidity.test.tsx packages/react/src/input/Input.test.tsx
```

Results: Field.Control 26 passed/one skipped; related suites 97 passed/four skipped. The skipped tests require browsers, which were unavailable. The original performance test was strengthened to count the actual render callback rather than only a wrapper component. Its default-mode steady-state assertion passed. The new onChange validation-count test passed. No independent performance measurement was attempted, and the reported extra onChange render is not escalated.

Scratch tests reside in `../repros` relative to this report directory. Their config uses jsdom, a symlink to the checkout's React package dependencies, and source aliases for Base UI React and utils. They run using the clone's binary with an external root:

```sh
TZ=UTC <clone>/node_modules/.bin/vitest run --root <work>/repros
TZ=UTC <clone>/node_modules/.bin/vitest run --root <work>/repros verified-observations.test.tsx
```

The initial four-test boundary suite had three intended head assertion failures and one passing base comparison. The final snapshot test passed and confirmed DOM empty, filled true, raw newline validation argument, and empty Form value. Two intermediate observation attempts needed correction: one incorrectly expected externally controlled Form errors to disappear; another accidentally used a literal JSX backslash-n attribute instead of a JavaScript newline string. Neither assumption is used as review evidence. The corrected final snapshot uses `value={'\n'}` and explicitly observes that controlled Form errors remain present.

The review is read-only. `git diff --exit-code` and `git status --porcelain=v1 --untracked-files=all` were clean after execution. No patch was applied and no proposed remedy was executed.
