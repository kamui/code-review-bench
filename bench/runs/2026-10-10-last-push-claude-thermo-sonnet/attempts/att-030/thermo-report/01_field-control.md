# Detail: Field.Control (`packages/react/src/field/control/FieldControl.tsx` and its test)

## Commands run
- `git diff main...review-head` (2 files, +197/-26).
- `TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field`: 10 files, 240 passed, 9 skipped. FieldControl.test.tsx: 27 passed, 1 skipped. Verification status: tests pass, so findings are structural/semantic reading, not failing tests. Browser mode was not available.

## A. Duplicate sync pipelines (summary finding 1)
Evidence: hook body lines 105-115; `onChange` lines 137-158; mount effect lines 99-103. Siblings (SwitchRoot lines 105-111) have a single `useValueChanged` body with exactly the clear/dirty/filled/change sequence and no second path, because the sibling's DOM event writes state that the hook observes. FieldControl is different only because uncontrolled input has no React state.

Worked code-judo proposal:
```ts
const syncFieldState = useStableCallback((next: string) => {
  clearErrors(name);
  setDirty(next !== (validityData.initialValue ?? ''));
  setFilled(next !== '');
  validation.change(next);
});
useValueChanged(serializedValue, () => { syncFieldState(serializedValue!); });
// onChange: if (!isControlled && !defaultPrevented && !details.isCanceled) syncFieldState(inputValue);
```
Note the current uncontrolled path calls `setDirty`/`setFilled` even when the change is prevented or canceled, and only clears errors and validates when it is not. A single helper makes that discrepancy visible and forces an explicit decision. Verification status: derived by reading; not executed.

## B. Controlled cancel/prevent (summary finding 2)
Evidence: line 144 early return precedes line 153 guard. The test "does not clear errors or validate when change is prevented" and the new "does not validate when the change is canceled" both render uncontrolled controls. No controlled variant exists. Reasoning: in controlled mode the hook fires from the prop change, with no access to the event details, so neither flag can affect it. Verification status: confirmed by reading the control flow; no scratch test was run.

## C. Description versus diff (summary finding 3)
Evidence: `grep -n "setFocused"` shows only lines 57, 121, 159 and 163; no unmount or disabled cleanup. No `processedValueRef` anywhere in the file. Line 110 calls `clearErrors(name)` in the hook. Verification status: confirmed by grep and reading.

## D. Serialized registration (summary finding 4)
Evidence: `FieldControl.tsx` lines 86-94; `internals/field-register-control/useFieldControlRegistration.ts` `getRegistrationValue` (prefers `registration.value`) used by `captureInitialValue` and `validate`. Verification status: confirmed by reading; behavior at submit was not executed.

## Minor
The existing test "avoids rerendering for uncontrolled input changes" was rewritten from a wrapper component to a `render` prop counter. This changes what it measures (the control's own renders versus the wrapper's). It is fine for the new controlled test, but the rewrite of an existing test should be explained in the PR.
