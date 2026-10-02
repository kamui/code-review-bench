# Event permission and field transitions

## Finding: Cancellation still performs a partial field update

`FieldControl.tsx:137–156` constructs change details and calls the consumer callback, but checks `details.isCanceled` only at line 153. Lines 149–150 have already called `setDirty` and `setFilled`. Thus the new cancellation branch gates error clearing and validation, not internal handling as a whole.

The contract is explicit in `createBaseUIEventDetails.ts`: `cancel` cancels Base UI handling of the event. Number Field's setter, at `NumberFieldRoot.tsx:245–255`, checks the same flag immediately after notification and returns before storing its value and setting dirty. This is useful local precedent for the transaction boundary; it does not require sharing Number Field's value model.

The new head test at `FieldControl.test.tsx:189–201` asserts only that the validator is not called. That assertion passes. In the scratch reproduction, an uncontrolled input begins empty and the callback always cancels. Changing the input to `a` skips validation but makes both root attributes `data-dirty` and `data-filled` present. The initial contract test fails at its dirty assertion; the final observation test independently confirms both attributes are true.

These dirty/filled effects occurred at the base too, because cancellation was ignored entirely. The finding is that the explicitly added cancellation fix preserves partial internal handling and gives it a misleading contract. It is not represented as a newly introduced data-dirty regression. The change under review introduces the cancellation gate, and its placement should be corrected as part of that fix.

## Worked cancellation remedy

Keep notification and field-state application as separate stages. The narrow repair is an early return immediately following `onValueChange`:

```tsx
const details = createChangeEventDetails(REASONS.none, event.nativeEvent);
onValueChange?.(inputValue, details);
if (details.isCanceled) {
  return;
}
if (isControlled) {
  return;
}
applyFieldValue(inputValue, !event.nativeEvent.defaultPrevented);
```

Here `applyFieldValue` is the ordered operation worked out in `01_value_synchronization.md`. This moves cancellation outside the operation and removes the possibility of placing it between setter calls. Native prevention retains the existing distinct behavior: it suppresses validation/error clearing, while the old handler still projected dirty and filled. Do not silently conflate native prevention with `details.cancel()`.

This sketch addresses canceled uncontrolled handling. The DOM is already changed when React calls onChange; the proposal does not promise that cancel rewinds a native uncontrolled input. Likewise, if a consumer both stores a controlled value and cancels the notification, deciding how an independently authoritative prop should behave requires an explicit contract. The reproduced finding does not depend on that mixed case: it uses only an uncontrolled callback that calls cancel.

Expand the new existing test to assert pristine/unfilled field state after cancel and check that Form errors are unchanged. Add a later accepted transition to ensure cancellation does not disable future changes or leave stale permission state. No cancellation remedy was applied or verified.

## Finding: The controlled echo bypasses native event prevention

At the base, native-prevented events still notify `onValueChange` and project dirty/filled but skip `clearErrors` and `validation.change`. The head keeps the corresponding guard at `FieldControl.tsx:153`; however, lines 142–145 return for controlled controls before reaching that guard. The controlled observer at lines 105–114 has no event permission and unconditionally validates an actual prop change.

The regression comparison uses exactly the pattern already exercised by the retained prevented-event test, with the missing controlled setup added. A Form contains a named Field with onChange validation; its control has `value={value}` and `onValueChange={setValue}`. A one-shot capture listener calls preventDefault on a cancelable native input event. The event callback echoes `a` to the controlled prop. The base module invokes the callback but does not call the custom validator. The head invokes that validator once with `a` and Form values `{ message: 'a' }`.

The scratch suite imports the historical FieldControl source copied from `git show main:packages/react/src/field/control/FieldControl.tsx`. Only its relative imports are rewritten to absolute checkout source paths; surrounding Field, Form, hooks, and registration remain the installed head environment. This is a focused comparison of the changed control, not a claim that the entire merge-base suite was run. The base assertion passes and the head assertion fails. The final head observation confirms the validator call.

Source inspection also shows unconditional `clearErrors(name)` in the controlled observer. The reproduction uses controlled Form errors, so those errors remain visible despite that call; clearing the displayed error is not claimed as demonstrated behavior. The confirmed regression is the forbidden custom validation call. A previously suppressed async validator or expensive validator can now run for the prevented native event.

The retained head prevented-event test covers only the uncontrolled control. Its passing result cannot establish this contract for controlled input, particularly now that those paths have different owners.

## Worked event-boundary proposal and constraints

Keep one synchronization owner for controlled values. Returning to eager validation in onChange and also observing props would recreate double validation and allow rejected or transformed event values into field state. That is not an acceptable remedy.

The missing input to the controlled owner is the permission associated with an immediate accepted input echo. Model that permission locally at the handoff from callback to prop observation. A bounded typed record is preferable to a global skip flag: for example, a pending native input transition with its proposed DOM value and `validationAllowed` boolean, consumed when the resulting controlled commit is observed. Independent programmatic prop updates carry no prevented event and must continue validating normally.

The handoff needs deliberate lifecycle rules. Record permission before invoking a callback that may synchronously commit through flushSync. Consume the record once. Clear it when the parent rejects an event so a later unrelated programmatic update is not suppressed. Account for consumer rewrites and multiple events batched into one commit. Match the accepted committed DOM value, not blindly the unaccepted event value. Do not leave a permanent `skipNextValidation` bit that is cleared only by the next value change; that turns rejection into a new stale-state bug.

These are implementation constraints, not a proven drop-in metadata algorithm. The exact handoff requires tests because Field.Control accepts ordinary React setters and consumers may defer updates. If native prevention is intentionally being redefined to apply only to uncontrolled inputs, document and test that contract change explicitly instead of retaining a mode-dependent workaround by accident. The current diff neither removes the stated workaround nor supplies such an explicit contract.

The code-judo part that is already concrete is to route both triggers through the same native-value projection and ordered field-update operation. Carry source permission only where it is needed, at the callback/controlled-commit boundary; keep shared `useFieldValidation` free of special cases for Field.Control. A package-wide event state machine would be disproportionate to this two-file change.

## Verification and test sequence

The commands and overall results are recorded in `01_value_synchronization.md`. The focused scratch assertions are:

```tsx
// Base passes; head fails after the same prevented controlled echo.
expect(validate).not.toHaveBeenCalled();

// Head skips validation but fails this canceled transition contract.
expect(root.hasAttribute('data-dirty')).toBe(false);
expect(root.hasAttribute('data-filled')).toBe(false);
```

`boundaries.test.tsx` has four tests: the prevented event at base, the prevented event at head, canceled dirty/filled handling at head, and sanitized-value consistency at head. It finishes with one pass and three failures. `verified-observations.test.tsx` passes and checks actual head outcomes: one validator call for prevention, both attributes true after cancellation, and the sanitized-value mismatch.

Before approving a repair, test cancellation in both value modes, prevented input in both modes, rejected and rewritten controlled events, ordinary programmatic reset, a prevented event followed by rejection and then a programmatic update, batched events, and a pending async validation for the old value. These are targeted behavior checks for the proposed boundary, not a request for a generic matrix across every sibling component. The shipped suites all pass, and browser-only paths remain unverified under the allowed offline execution policy.
