# Event cancellation and scope contracts

## Finding: check cancellation before metadata updates

In `packages/react/src/field/control/FieldControl.tsx:148–155`, `details.isCanceled` is checked only after `setDirty` and `setFilled` have already run. Calling `details.cancel()` on an uncontrolled edit from an empty input therefore leaves `data-dirty` and `data-filled` set, even though validation is skipped; the scratch test observed both attributes becoming true. `setDirty(true)` also permanently marks the field as previously changed through `markedDirtyRef`, so this is more than a cosmetic partial update. The added cancellation test at `FieldControl.test.tsx:189–201` checks only validator calls and misses these writes. Return on cancellation immediately after the callback and before any internal field update, and test metadata as well as validation. Preserve the existing distinction that native prevention suppresses validation while cancellation stops Base UI’s internal handling.

The head creates a details object at `FieldControl.tsx:139`, calls the consumer at line 140, and returns for controlled inputs at `144–145`. In the uncontrolled path it writes dirty and filled at `149–150`, then checks cancellation at `153`. Thus the new cancellation check does not guard the whole internal operation.

The shared contract is explicit: `createBaseUIEventDetails.ts` documents `cancel` as “Cancels Base UI from handling the event.” Its implementation sets an `isCanceled` getter; it does not itself roll back native DOM changes. This review requests that Base UI stop its own state writes, not that it undo browser input mutation. Switch provides an existing straightforward admission pattern: `SwitchRoot.tsx:193–199` checks cancellation immediately after the callback and before changing its state.

There is a persistent hidden write as well as visible attributes. `FieldRoot.tsx:69–77` sets `markedDirtyRef.current = true` when dirty becomes true, and does not reset it when dirty later becomes false. `useFieldValidation.ts:223–228` uses this history to decide whether an empty required control should show an error. A canceled operation therefore changes future validation policy even while skipping the immediate validator call.

The new cancellation test, `FieldControl.test.tsx:189–201`, passes because it asserts only `validate` was not called. The scratch test also observes zero calls, then asserts `{ dirty: false, filled: false }`. It instead receives `{ dirty: true, filled: true }`. This is an incomplete fix to behavior the PR explicitly sets out to support; it is not presented as a formerly working cancellation behavior broken by the patch.

## Worked proposal: decide admission before doing work

The cancellation remedy can delete the late cancellation condition rather than layering on compensating state restoration:

```tsx
onChange(event) {
  const inputValue = event.currentTarget.value;
  const details = createChangeEventDetails(REASONS.none, event.nativeEvent);
  onValueChange?.(inputValue, details);

  if (details.isCanceled) {
    return;
  }

  if (isControlled) {
    // Retain policy for the accepted controlled commit, as described in detail 01.
    return;
  }

  applyFieldValue(
    inputValue,
    event.nativeEvent.defaultPrevented ? 'preserve-validation' : 'validate',
  );
}
```

`applyFieldValue` is the single local transaction sketched in detail 01. The ordering makes the invariant explicit: canceled proposals never enter it. This removes the partial-update behavior and the extra cancellation clause buried beside validation. A compensating `setDirty(false)` would be wrong: it would erase preexisting dirty state and would not undo the sticky markedDirtyRef. Early admission avoids that entire category of bookkeeping.

The controlled consumer still owns its committed value. If a consumer both calls `cancel()` and independently sets a new `value` prop, the accepted prop update needs an explicit policy; it must not be handled by validating the rejected raw DOM proposal. This report does not assume that cancellation should roll back a consumer’s own state update. The confirmed cancellation finding requires no such ambiguity: its reproduction is uncontrolled and calls only `cancel()`.

Do not replace native prevention with cancellation globally. The preexisting native-event workaround intentionally still calls the consumer and updates dirty/filled, while suppressing error clearing and validation. The shared transaction proposal preserves that distinction and prevents future code from silently conflating these policies.

## Verification status and test remediation

The relevant existing and scratch command lines and totals are preserved in detail 01. The scratch test’s exact failed comparison is:

```text
Expected: { dirty: false, filled: false }
Received: { dirty: true, filled: true }
```

The assertion that validation had zero calls passed first. The production code’s sticky markedDirtyRef behavior is established by source inspection; no separate runtime test of its later required-error effect was executed.

Strengthen the cancellation test with field metadata assertions. Cover cancellation from both initially empty and already dirty/filled states, so the remedy preserves the previous metadata rather than blindly clearing it. A further required-field check should confirm that a canceled operation does not count as accepted change history. Retain the existing native-prevention test and add its controlled counterpart. Keep the accepted-rewrite, rejection, async reset, and rendering checks described in detail 01.

No browser execution, network access, or source edits were performed. The sketches are proposed remedies and have not been validated as patches.

## Scope question, not an actionable regression finding

Is focused-state cleanup still intended in this PR? The packet’s PR body promises that unmounting or disabling a focused control will clear `data-focused`, but the pinned diff only relocates the autofocus effect at `packages/react/src/field/control/FieldControl.tsx:119–123`; it adds no focus cleanup. This is a discrepancy between the stated scope and the implementation, not a newly introduced regression. Either implement and verify that promised behavior or update the PR description.

The autofocus effect has no cleanup function at head, just as at base. The `onBlur` path clears focus, but unmount does not add that path. The description therefore should not be treated as proof that the fix exists. Browser behavior when disabling a focused input was not tested; this question rests on the absence of the promised implementation, not a claimed browser reproduction.

Other packet-description differences are relevant to understanding the reviewed head: it contains no `processedValueRef`; instead controlled events return and the prop observer drives field updates. It does call `clearErrors(name)` in that observer, so the body’s open question about omitting error clearing does not describe the pinned source. Neither discrepancy is itself an additional finding.

## Maintainability judgment

The change does not warrant a broad control framework, a generic state machine, or test-file decomposition. The structural improvement that matters here is a single coherent field-update transaction and an explicit event admission boundary. Keep uncontrolled state driven by the DOM, keep controlled state driven by accepted props, and keep error/validation policy attached to the originating update where it can be determined. Passing the current tests is insufficient while these contracts disagree.

Final checkout identity verification is recorded in detail 01. This report is a source review with focused reproductions, not an implemented refactor.
