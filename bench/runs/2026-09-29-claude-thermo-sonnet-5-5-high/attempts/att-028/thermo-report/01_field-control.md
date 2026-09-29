# Detail: `Field.Control` controlled sync (packages/react/src/field/control)

## Scope and commands

- `git diff main...review-head -- packages/react/src/field/control/FieldControl.tsx` and `FieldControl.test.tsx`.
- `TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx` gave 26 passed, 1 skipped (the SSR autoFocus test is skipped in jsdom).
- Read: `internals/useValueChanged.ts`, `internals/field-register-control/useRegisterFieldControl.ts`, `field/root/FieldRoot.tsx` (`setDirty`, `markedDirtyRef`), `field/root/useFieldValidation.ts` (`change`), and `number-field/input/NumberFieldInput.tsx` (the sibling `useValueChanged` usage).

## Measurements

- `FieldControl.tsx` is 207 lines at head, so the 1k rule is not in play.
- Writers of `setFilled` at head: mount effect (line 99-103), `useValueChanged` callback (line 112), `onChange` (line 150). Writers of `setDirty`: lines 111 and 149. Callers of `clearErrors` + `validation.change`: lines 109/114 and 154/155.

## Finding 1 detail: duplicated pipeline

The two copies (head line numbers):

    useValueChanged (105-115):  clearErrors -> setDirty(cmp) -> setFilled(!== '') -> validation.change
    onChange (143-157):         setDirty(cmp) -> setFilled(!== '') -> [guard] clearErrors -> validation.change

Note the ordering also differs: the controlled copy clears errors before setDirty, the uncontrolled copy after. `validation.change` reads `markedDirtyRef`, which `setDirty(true)` sets synchronously (`FieldRoot.tsx` `setDirty`), so the order of `setDirty` before `validation.change` matters in both copies. The order of `clearErrors` relative to them appears not to matter. That is one more reason the sequence should not be hand-written twice.

### Worked code-judo proposal

    const syncValue = useStableCallback((next: string, validate: boolean) => {
      // dirty must be updated before validation: validation.change reads markedDirtyRef
      setDirty(next !== (validityData.initialValue ?? ''));
      setFilled(next !== '');
      if (validate) {
        clearErrors(name);
        validation.change(next);
      }
    });

    useValueChanged(serializedValue, () => {
      if (serializedValue !== undefined) syncValue(serializedValue, true);
    });

    onChange(event) {
      const details = createChangeEventDetails(REASONS.none, event.nativeEvent);
      onValueChange?.(event.currentTarget.value, details);
      if (isControlled) return;
      syncValue(event.currentTarget.value, !event.nativeEvent.defaultPrevented && !details.isCanceled);
    }

If cancel is meant to stop dirty and filled too, the helper should be skipped entirely when canceled. The helper makes that a one-line decision in one place.

## Finding 2 detail: cancel and preventDefault

- Uncontrolled (lines 148-157): `setDirty` and `setFilled` execute before the `!defaultPrevented && !details.isCanceled` check. `details.cancel()` therefore does not prevent the field from becoming dirty and filled. The added test "does not validate when the change is canceled" asserts only that `validate` is not called. It does not assert `data-dirty` or `data-filled`, which is why the gap passes.
- Controlled (lines 144-146): returns after `onValueChange` with no `defaultPrevented` check. Prior behavior: `onChange` skipped `clearErrors` and `validation.change` for a prevented event regardless of control mode. New behavior: if the consumer stores the value, `useValueChanged` runs clearErrors and validation with no reference to `defaultPrevented`. The comment at the top of the guard cites React issue 9023, which is about exactly this event, so dropping it for the controlled path deserves an explicit decision. The existing prevented-change test is uncontrolled, so it does not cover the difference.
- Unverified by execution: I did not write a scratch test for the controlled preventDefault case.

## Finding 3 detail: filled state

Before the PR the filled logic was one layout effect keyed on `valueProp`. It now has three triggers. The mount effect (`if (inputRef.current?.value) setFilled(true)`) is DOM-based and never clears. `useValueChanged` does not fire on mount, so mount-time controlled values with an empty string rely on filled starting `false`. That holds, and the test "does not set filled state on mount for an empty controlled value" covers it. Correct, but the correctness depends on three separate pieces agreeing.

## Finding 4 detail: `String(value)` registration

`useRegisterFieldControl` stores `value` in the registration next to `getValue: getValueFromInput`. `getValue` returns the DOM string, so form value collection likely uses the DOM string either way, which limits the exposure to whichever reader uses `registration.value` directly (initial value baseline, reset). I did not trace every reader. `validityData.initialValue` is now a string for numeric controlled values, which is the intended fix. For array values `String(['a','b'])` is `"a,b"`. React renders an array `value` on a text input as that same joined string, so the DOM and the baseline agree. The remaining risk is only for readers that expect the original type. There is no Form-level test on the serialized shape.

## Finding 5 detail: diff noise and tests

- `inputRef` and the autofocus effect are moved below `useValueChanged` with no functional dependency.
- The first render-count test was changed from a wrapper component to a `render` prop counter. That is a reasonable change, since a wrapper component does not observe the control's own re-renders, but it should be called out as a test-methodology change.
- Test title "renders once per keystroke" vs. PR table (two per keystroke in `onChange` mode; the test itself uses the default `onSubmit` mode, where steady state is one). The title is accurate for that mode but does not say so.

## Verification status

- Executed: existing and new FieldControl tests pass in jsdom.
- Read-only inference (not executed): controlled `preventDefault` behavior change, `registration.value` consumers, and the ordering claim about `clearErrors`.
