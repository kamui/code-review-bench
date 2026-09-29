# Field control synchronization

## Scope and evidence

Reviewed `packages/react/src/field/control/FieldControl.tsx` and the adjacent control and field registration patterns. The change makes serialized controlled values the registration baseline, routes controlled value changes through `useValueChanged`, and leaves uncontrolled changes in the DOM `onChange` handler. This is a reasonable ownership split and does not create a large-file or decomposition concern.

The cancellation flow has a gap. In `onChange`, `onValueChange` receives `details`; controlled mode then returns at lines 142–145, without examining `details.isCanceled`. For uncontrolled mode, `setDirty` and `setFilled` run at lines 148–150, before the only cancellation check at lines 153–155. That check suppresses only `clearErrors` and `validation.change`. The layout-effect path at lines 105–115 synchronizes any subsequent controlled prop change without knowing whether the event that produced it was canceled. Consequently, the same canceled callback can leave dirty/filled state changed in uncontrolled mode or still clear errors and validate after an accepted controlled echo.

This conflicts with the event-details contract in `createBaseUIEventDetails.ts`: `cancel()` cancels Base UI's internal handling. Neighboring controls such as Checkbox check `details.isCanceled` before committing internal state. The added cancellation test at `FieldControl.test.tsx` only asserts that validation did not run; it does not assert dirty/filled state or exercise the controlled echo path.

## Finding

In [FieldControl.tsx](/home/jack/.t3/bench-runs/2026-09-29-codex-thermo-high/att-022/clone/packages/react/src/field/control/FieldControl.tsx#L137), `details.cancel()` is checked only after the uncontrolled path has already updated dirty and filled state, while controlled changes return before checking cancellation at all. A canceled change can therefore still change `data-dirty`/`data-filled`; in controlled mode, a parent that accepts the proposed value causes the later prop-sync effect to clear errors and validate it. Handle cancellation as one event policy before either path mutates field state, and carry that decision through controlled synchronization if the accepted prop echo is meant to remain canceled.

## Worked code-judo proposal

Keep the two ownership paths, but make the event decision explicit before branching. After invoking `onValueChange`, return immediately for a canceled event before any uncontrolled field-state mutation. For the controlled path, the prop effect cannot infer event cancellation from the value alone; preserve an event-scoped canceled value/revision so the matching prop echo is consumed without Field-owned validation or error clearing. Independent later prop changes must still use the ordinary controlled synchronization path. This avoids trying to infer whether a prop update came from the canceled event based only on equality, and keeps event cancellation separate from normal controlled-value synchronization.

A focused regression should cover both modes: cancel a change and assert field attributes, errors, and validation remain unchanged; for controlled input, have the callback accept the proposed value and confirm the echo does not re-run Field's canceled user-change effects. Also retain a test that an unrelated programmatic controlled update continues to synchronize.

## Verification status

Executed from the clone root:

`TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx`

Result: 1 test file passed; 26 tests passed and 1 skipped. The suite's current cancellation test verifies only that validation is not called. Browser-based verification was unavailable under the run policy; no browser rendering conclusion is needed for this finding.
