# Field control state and event flow

## Scope and evidence

The implementation change is in `packages/react/src/field/control/FieldControl.tsx` (207 lines after the change; no 1,000-line threshold concern). The new `serializedValue` is registered as the baseline-compatible value, and `useValueChanged` handles controlled prop changes by clearing errors, setting dirty/filled state, and calling `validation.change`. The input event handler creates change details and calls `onValueChange`; at lines 144–146 it returns immediately for controlled inputs. The native `defaultPrevented` and details cancellation checks occur only at lines 153–155, below that return.

The shared event-details contract in `createBaseUIEventDetails.ts` describes `cancel()` as canceling Base UI's internal handling. Existing sibling control handlers check `details.isCanceled` before continuing their internal handling. The existing Field.Control test at `FieldControl.test.tsx:203–227` verifies that a prevented uncontrolled change neither clears a server error nor validates, while the new cancellation test checks only validation on an uncontrolled input. There is no controlled case for either suppression mechanism.

## Finding: controlled change cancellation is ignored

The early return means a controlled `onValueChange` callback can call `details.cancel()` and still trigger internal handling if it updates the controlled value. The same gap exists when the native event was prevented: the controlled prop effect has no event context and always runs `clearErrors`, dirty/filled updates, and `validation.change`. This makes controlled Field.Control inconsistent with its uncontrolled behavior and the change-detail contract. It can clear a server error and start validation for an event that the consumer explicitly canceled or prevented.

## Worked code-judo proposal

Keep the controlled prop as the authority for the displayed and accepted value, but make the event-to-prop echo explicit. Record the proposed controlled value together with whether that event was canceled or prevented before returning from `onChange`. When `useValueChanged` observes that exact echo, it should still allow the accepted prop to render and register, but skip only the internal event work that was suppressed: clearing errors and validating. A later prop change that does not match a recorded event echo remains a programmatic value change and runs the normal synchronization. Clear the recorded marker after it is consumed so a repeated value cannot accidentally inherit stale suppression.

Alternatively, centralize state synchronization behind a helper that accepts the value and an explicit `shouldValidate`/`shouldClearErrors` decision, with the controlled echo carrying that decision. Avoid simply moving the checks into a shared callback without passing the event decision: the effect cannot infer `defaultPrevented` or cancellation after the native event handler returns.

## Verification status

Ran the permitted focused test command from the clone root:

`TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx`

Result: 1 test file passed; 26 tests passed and 1 was skipped. The existing cancellation coverage is uncontrolled only; no controlled regression test was present. `git diff --check main...review-head` passed. No browser-rendered behavior was inspected because browser execution is unavailable under the run policy.
