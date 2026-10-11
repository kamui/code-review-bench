# Field value synchronization and event policy

## Source evidence

The head adds `serializedValue = value == null ? undefined : String(value)` at FieldControl.tsx:85–86 and passes it as the registration value at :90–97. `useValueChanged(serializedValue, ...)` at :105–115 performs `clearErrors`, dirty comparison, filled comparison, and `validation.change`. The uncontrolled event handler at :148–155 performs the same operations, but skips error clearing and validation for a prevented native event or canceled event details. The controlled branch returns before that gate at :142–146.

This review treats these two paths as adapters for one accepted-value transition. The controlled prop is authoritative, which is an architectural improvement over validating every attempted DOM change. The new validator-once and render-count tests protect useful behavior. The split implementation nevertheless makes event policy incidental to the adapter: a prevented event validates if controlled, but not if uncontrolled. Cancellation is applied after half of the internal transition, and a second value representation is used for controlled validation.

`useValueChanged.ts:6–16` initializes its reference to the incoming value, invokes its callback only after a changed value commits, and uses an isomorphic layout effect. It neither carries event metadata nor runs for the initial value. The ownership issue belongs to FieldControl, not to this generic observer.

`useFieldControlRegistration.ts:35–49` already reads a live value through `registration.getValue` for form values. `getRegistrationValue` falls back to that getter when `registration.value === undefined`. That function owns both initial-value capture at :92–101 and imperative validation at :52–61. The PR's explicit serialized registration value bypasses the live getter for those latter purposes even though form values still use it.

`useFieldValidation.ts:255–262` obtains each named form value from its getter, then calls `validate(value, formValues)`. Passing a coerced prop as `value` while collecting the same field through a native getter permits the validator's two arguments to disagree about that field. `change` at :316–328 also owns debounce cancellation and validation sequence invalidation, so a corrected adapter should continue calling it rather than duplicating that machinery.

`FieldRoot.tsx:69–78` synchronously sets `markedDirtyRef.current = true` for a truthy dirty update. `useFieldValidation.ts:223–228` uses that reference to decide whether a missing required value should become an error. Cancellation after `setDirty` affects validation history even if the dirty attribute is subsequently reset.

`createBaseUIEventDetails.ts:65–68` documents cancellation as canceling Base UI handling. Its cancel method sets a flag; it does not undo updates or prevent the native event. Moving the cancellation gate earlier is therefore required if the new feature is intended to stop the full internal transition. It need not restore the browser's DOM edit to satisfy this internal-state requirement.

## Finding: native prevention is lost for controlled updates

FieldControl.tsx:110–114 applies controlled validation unconditionally after a changed serialized prop. A capture listener can prevent a native input event while React still calls the change handler. The callback then updates the controlled value, and the effect validates it without the prevented-event information. The old event handler explicitly gated validation on `event.nativeEvent.defaultPrevented`; the new uncontrolled handler retains that gate. The refactor has made the component's controlled mode determine whether prevention is respected.

The scratch test `preserves native prevention for an echoed controlled value` uses the existing test's cancelable native-input technique with a controlled state owner. The callback is called once, as expected. On the head the validator is unexpectedly called once with `['a', { message: 'a' }]`. The test's assertion that the displayed server error remains present passes; the observed failure is validation, and this report does not claim that the error necessarily disappears in this sequence. `clearErrors` is nevertheless invoked by the source path, which makes its policy divergence worth fixing alongside validation.

The same probe passes when the baseline FieldControl is substituted. Verification status: reproduced in jsdom and differential baseline checked. No browser behavior is inferred beyond the tested React/native-event sequence. The changed-line anchor is FieldControl.tsx:110–114.

## Finding: string coercion is not the native value boundary

FieldControl.tsx:85–97 and :105–114 equate JavaScript coercion with the string value of a native input. For a text input, a controlled `'a\n'` prop yields DOM value `'a'`. The scratch test `validates the DOM value when a controlled consumer appends a newline` verifies that the control contains `'a'` and the validator's `formValues` argument is `{ message: 'a' }`; its first argument is nevertheless `'a\n'` on the head. The old handler passes the DOM event value `'a'` and the baseline probe passes.

The companion probe begins with controlled `'ab'` and programmatically assigns `'a\nb'`. The displayed value remains `'ab'`, but the head marks the root dirty because it compares raw string representations. The baseline has no change event and remains pristine. Together these examples show both a value-contract split and a dirty-state regression. They do not require invalid controlled types, undefined/null mode switching, or a browser-only number-input edge case.

Verification status: both failures reproduced in jsdom and both pass with the baseline control. Other input types also have their own normalization rules, but no unexecuted type-specific scenario is counted as an additional finding. The changed-line anchor is FieldControl.tsx:85–86.

## Finding: cancellation happens after internal state mutation

FieldControl.tsx:153–155 adds `!details.isCanceled` to the validator/error gate, but dirty and filled have already changed at :149–150. The scratch probe calls `details.cancel()` on an initially empty uncontrolled control. The validator is correctly skipped; both root attributes are incorrectly set under a full-cancellation contract. The new repository test at FieldControl.test.tsx:189–201 asserts only that validation was skipped, leaving those mutations invisible.

Verification status: the partial update is reproduced on the head, with two failing soft assertions for dirty and filled. There was no differential baseline cancellation run because the base lacks cancellation support entirely, and this is explicitly not classified as a new dirty/filled regression. The source proves the new cancellation branch is non-atomic. The changed-line anchor is FieldControl.tsx:153–155. Fix the added support by returning on cancellation immediately after the callback, before internal state writes.

## Worked code-judo proposal

Keep two observation adapters, because controlled values and uncontrolled DOM values have different owners. Delete the two copies of the transition policy. A local function can own the accepted native value and the required ordering:

```tsx
const applyAcceptedInputValue = useStableCallback(
  (next: string, shouldValidate: boolean) => {
    setDirty(next !== (validityData.initialValue ?? ''));
    setFilled(next !== '');

    if (shouldValidate) {
      clearErrors(name);
      validation.change(next);
    }
  },
);
```

The boolean expresses the existing native-prevention rule: dirty/filled may reflect the edit while validator/error handling is suppressed. Cancellation has a different meaning and should return before calling this function. This is a local operation with one domain invariant, not a generic policy object or a wrapper that only forwards another helper.

Registration can then use its existing native getter boundary directly:

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

The registration getter supplies the original native baseline and the current native value for imperative validation. It remains live even without re-registration on every value change. The field's existing registration effects continue synchronizing validity metadata. This removes the parallel prop-value contract rather than centralizing string coercion into a new serializer.

The controlled observer may retain the serialized prop solely as a change notification key, but should obtain `next` from `getValueFromInput()` after commit before applying it. The string key keeps equivalent number/string/array representations from automatically generating duplicate observations; the native getter decides the value. Guard a missing input before applying state rather than treating a missing ref as an empty edit. Evaluate whether an unchanged native value should validate based on the established programmatic-change contract; do not silently conflate that with string-key equality.

For the uncontrolled event adapter, call the consumer, return immediately when `details.isCanceled`, then apply the DOM value with `shouldValidate = !event.nativeEvent.defaultPrevented`. For a controlled event adapter, continue waiting for the accepted prop rather than applying the attempted DOM value. Carry only the event disposition needed for the resulting controlled commit. Associate it with the echo and consume/expire it so a rejected event does not suppress a later independent reset. The event-to-commit association requires explicit lifecycle reasoning, including delayed owner updates and repeated events; adding a permanent skip boolean is not an adequate fix.

This proposal preserves the valuable accepted-prop ownership and uses the canonical `validation.change` and registration getter. It removes the duplicated dirty/filled/error/validation sequence and the independent controlled baseline representation. It deliberately does not revive the PR body's event-first/processed-value-ref design: validating the attempted event value would reintroduce state for rejected or rewritten edits. No code changes or proposed-refactor tests were executed; this is a worked design proposal, and the exact event-disposition implementation must be validated before adoption.

## Executed verification

From the clone root, the following command ran once and passed 26 tests with one skip:

```sh
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx
```

An independent selection ran once and passed 139 tests with three skips:

```sh
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/input/Input.test.tsx packages/react/src/field/root/FieldRoot.test.tsx packages/react/src/form/Form.test.tsx
```

Let `CLONE` denote the supplied clone path and `PROBES` the sibling `clone-work/thermo-report/probes` directory. These names describe command locations; they were not environment overrides. The external head command was:

```sh
TZ=UTC <CLONE>/node_modules/.bin/vitest run --root <PROBES>
```

The config uses jsdom, includes only `*.test.tsx`, and maps the Base UI imports into the clone's source. Its node_modules link points to the installed packages/react dependencies. The run finished in 1.38 seconds with five failing and three passing tests. Raw output is [probes/run.log](probes/run.log), and the test source is [probes/field-boundaries.test.tsx](probes/field-boundaries.test.tsx). Passing probes confirm rejected controlled input does not validate, uppercase rewriting validates the accepted value once, and a programmatic change invalidates an old pending asynchronous validation result.

For the differential check, `git show main:packages/react/src/field/control/FieldControl.tsx` provided the baseline component. A scratch-only transformation rewrote its relative imports to their absolute paths in the clone. A resolver plugin substitutes that file for the Field parts export. All other implementation dependencies are unchanged by the reviewed range, verified by the committed diff. This comparison isolates the component change and does not claim to run a complete baseline tree.

```sh
TZ=UTC <CLONE>/node_modules/.bin/vitest run --root <PROBES> --config <PROBES>/vitest.base.config.mts --testNamePattern 'preserves native|validates the DOM|keeps dirty|initializes filled'
```

That selection ran once, finished in 911 ms, and passed all four selected probes; the four unselected probes were skipped. The baseline source, configuration, and output are retained in [probes/base-FieldControl.tsx](probes/base-FieldControl.tsx), [probes/vitest.base.config.mts](probes/vitest.base.config.mts), and [probes/base-run.log](probes/base-run.log). No network, registry, browser, ambient instructions, or external reviews were used.
