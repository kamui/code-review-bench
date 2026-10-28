## Findings

**P2 — Controlled changes bypass cancellation and native prevention**  
File: `packages/react/src/field/control/FieldControl.tsx`, lines 137–155.

For a controlled input, `onChange` calls `onValueChange` and returns at lines 139–145 before checking `details.isCanceled` or `event.nativeEvent.defaultPrevented`; the only checks are on the uncontrolled path at lines 152–155. If the controlled callback updates the value despite calling `details.cancel()`, or a capture listener prevents the native input event while the callback updates the prop, the new `serializedValue` reaches the `useValueChanged` effect at lines 105–115. That effect unconditionally clears form errors, updates dirty/filled state, and calls `validation.change`. This violates the event detail's documented cancellation contract and the existing Field behavior for prevented changes: canceled/prevented user input can clear a server error and publish validation results. The controlled synchronization needs to distinguish programmatic prop changes from a prop echo produced by a canceled or prevented input event.
