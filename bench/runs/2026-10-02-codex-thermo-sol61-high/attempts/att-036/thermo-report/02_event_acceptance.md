# Event acceptance and field state

## Cancellation is only partly implemented

`FieldControl.tsx:139–140` now retains change details, and line 153 checks `details.isCanceled`. That check occurs after the uncontrolled handler updates dirty and filled at lines 149–150. The API's existing change-details contract in `createBaseUIEventDetails.ts:65–68` says cancellation stops Base UI handling. The PR body also explicitly includes stopping internal handling as one of its fixes. Suppressing only validation does not meet that boundary.

In the scratch cancellation check, the initial field has no dirty or filled attribute, and `onValueChange` calls `details.cancel()`. After changing the input to `"a"`, the validator remains uncalled, but both dirty and filled are true. The combined assertion shows `{ dirty: true, filled: true }` against the expected false/false pair. `FieldRoot.tsx:69–76` additionally sets `markedDirtyRef.current = true` on the dirty update and never clears that history when subsequently setting dirty false. This history participates in required-error handling. The visible state discrepancy is tested; the downstream required-error effect is source evidence, not a separately executed scenario.

The added cancellation test at `FieldControl.test.tsx:189–200` observes only whether `validate` was called. It consequently passes while the all-state cancellation invariant fails. The base ignored cancellation entirely; this finding does not mislabel the dirty update as a new behavior. It identifies the incomplete fix and partial state transition introduced by the newly added cancellation gate.

## Worked cancellation remedy

Resolve cancellation before mutating field state. Keep it distinct from native `defaultPrevented`, which historically suppressed error clearing and validation while allowing dirty and filled to track the DOM edit. The minimal shape is:

```tsx
const inputValue = event.currentTarget.value;
const details = createChangeEventDetails(REASONS.none, event.nativeEvent);
onValueChange?.(inputValue, details);

if (isControlled || details.isCanceled) {
  return;
}

synchronizeFieldValue(inputValue, !event.nativeEvent.defaultPrevented);
```

`synchronizeFieldValue` is the local transition worked through in the companion report. This structure eliminates the late cancellation special case and makes cancellation an acceptance boundary. It does not attempt to roll back the browser's uncontrolled DOM edit; the finding is about Base UI's internal handling. For controlled fields, the consumer's committed prop remains authoritative, so a later independent prop update must still synchronize normally.

Extend the cancellation test to assert all internal effects together, including dirty and filled and preservation of an existing error. Keep a separate native-prevention test to prevent accidental conflation of the two policies. This proposal was not applied or executed.

## Controlled effects discard native prevention

The base's handler guarded `clearErrors` and `validation.change` with `!event.nativeEvent.defaultPrevented` for both modes. The head retains that guard at `FieldControl.tsx:153`, but only uncontrolled changes reach it: lines 142–145 return immediately for controlled fields. The accepted value then reaches the effect at lines 105–114, which clears errors and validates without the event's policy.

The scratch reproducer adapts the existing uncontrolled prevention test into a controlled component using `value={value}` and `onValueChange={setValue}`. A capture listener cancels the native input event with `preventDefault()`. The Form error object is stable across rerenders so the test measures clearing rather than error reinjection by a newly created prop. On the head, the validator is called once and the server error disappears. Against the base implementation, the validator is called zero times and the error remains. The focused assertion reports head `{ validations: 1, serverErrorPresent: false }` versus expected/base `{ validations: 0, serverErrorPresent: true }`.

This is a reproduced change in an existing guarded behavior. It is not a complaint about validating ordinary programmatic changes: those have no prevented originating input event and should take the new effect path. The existing prevention test at `FieldControl.test.tsx:203–225` covers only an uncontrolled input, so it cannot detect losing the guard when event handling delegates to the controlled effect.

## Worked prevention remedy and design constraints

Preserve one owner for controlled state. Keep dirty and filled driven by the accepted committed value. Carry the validation policy of a prevented event across the synchronous controlled echo instead of moving validation back into `onChange` and reintroducing two owners.

A concrete local design uses a pending event record containing the native event, the controlled value before the event, and a unique record identity. Record it before invoking the consumer, so a consumer that commits synchronously can be associated with the originating event. The controlled observation callback receives the previous value from `useValueChanged`; when that previous value matches the pending event's starting value, consume the record and use its native event's `defaultPrevented` state to select `runValidation`. Clear the record after consumption and expire it at the end of the event turn using an identity-checked microtask. An event that the consumer rejects must leave no suppression token capable of vetoing an unrelated later prop change.

The existing useValueChanged callback already provides the previous value, so this does not require teaching that generic hook about DOM events. The metadata belongs in FieldControl, where event acceptance and the controlled echo meet. The all-state transition can then remain simple: dirty first, filled second, and error clearing plus validation only when policy allows. This avoids scattering prevention checks into registration or field validation and preserves the ordering required by `markedDirtyRef`.

This is an unimplemented design proposal, not verified production code. Its event-turn lifetime treats a later independent prop update as programmatic; delayed/transitioned consumer updates require explicit tests to settle their association with the prevented event. The remedy must cover synchronous echo, consumer rewrite, rejection followed by an unrelated update, and callback-triggered synchronous commits. A blanket sticky `skipNextValidation` boolean is insufficient because the rejected event may produce no next controlled change. The reproduced prevention regression is established independently of which bounded association implementation is selected.

## Verification record

All scratch sources are in [field-review-tests](../field-review-tests/). The harness's five checks exercised cancellation state, controlled native prevention, sanitized accepted values, rejected controlled edits, and obsolete async validation. Rejection and async invalidation passed. The three boundary checks failed. No extra repository files or dependencies were modified.

The base component was generated with `git show main:packages/react/src/field/control/FieldControl.tsx`, with only import paths rewritten to resolve the unchanged source dependencies. The two selected regression checks passed under `FIELD_CONTROL_REVISION=base`. The original full scratch prevention check used an inline errors object; recreating it on every parent render reinjected the server error and masked clearing, though validation still failed. The harness was corrected to use a stable object, and the focused head/base pair was then executed:

```sh
TZ=UTC ../clone/node_modules/.bin/vitest run --root ../clone-work/field-review-tests --testNamePattern 'controlled native prevention' --reporter=verbose
TZ=UTC FIELD_CONTROL_REVISION=base ../clone/node_modules/.bin/vitest run --root ../clone-work/field-review-tests --testNamePattern 'controlled native prevention' --reporter=verbose
```

The head failed with both unexpected validation and error removal; the base passed. The correction improves the measurement and is retained in the final scratch source. Commands used the already installed binaries and no network. All executions were well below the five-minute limit. Browser-only behavior and the proposed remedies remain unverified.
