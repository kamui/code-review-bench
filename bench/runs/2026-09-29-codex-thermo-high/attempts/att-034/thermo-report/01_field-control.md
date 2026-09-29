# Field control synchronization

## Scope and measurements

The reviewed source change is `packages/react/src/field/control/FieldControl.tsx`, with focused coverage in `packages/react/src/field/control/FieldControl.test.tsx`. The source patch adds 48 lines and removes 20; the test patch adds 149 lines and removes 6. The resulting files are 207 and 273 lines, respectively. No file-size or decomposition concern is present.

The implementation converts the controlled value to a string before registering it and comparing it with the input's DOM string value (`FieldControl.tsx:83-97`). It reads a prefilled DOM value on mount for the uncontrolled case (`:99-103`), runs prop-driven synchronization through `useValueChanged` (`:105-115`), and returns early from the DOM change handler when the control is controlled (`:137-156`). This gives each mode one owner for validation and state updates: controlled updates follow the accepted prop value, while uncontrolled updates follow the native change event.

## Structural assessment

The main code-judo opportunity was to collapse controlled and uncontrolled synchronization into one shared transition routine. That would require adding mode and event-cancellation inputs to a routine whose call sites have different semantics: the uncontrolled handler must respect the native event's `defaultPrevented` flag and `details.cancel()`, while the controlled effect must only react to a value the consumer actually accepts. Keeping these ownership boundaries visible is simpler than centralizing them behind another conditional abstraction.

The dirty and filled comparisons appear in both paths, but their duplication is limited to two direct assignments and is consistent with the intentional separation of event-owned and prop-owned updates. Extracting these few statements would add another helper boundary without removing the meaningful branch or reducing the concepts a reader must track. The rest of the component remains a linear set of input handlers and hooks. No ad-hoc mode flags, type casts, or feature logic in a shared utility were introduced.

The source change also makes the event detail cancellation check explicit in the uncontrolled path (`FieldControl.tsx:152-155`). The controlled path does not synchronously update field state from the event, so there is no parallel event-side validation path to cancel; a subsequent accepted prop change is the authoritative controlled update. The new tests cover cancellation for the uncontrolled path, accepted controlled changes, numeric dirty comparisons, programmatic updates, initial filled state, and validation (`FieldControl.test.tsx:47-200`). I found no maintainability defect with a sufficiently concrete, behavior-preserving remedy to report.

## Verification status

The permitted focused jsdom unit suite passed:

`TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx`

Result: 1 test file passed; 26 tests passed and 1 was skipped. `git diff --check main...review-head` also completed without reporting whitespace errors. Browser-only behavior was not verified because browser execution is unavailable under this run's policy.

## Worked code-judo proposal

No restructuring is recommended. The most plausible simplification—making both modes call a common state-sync helper—would still need mode-specific gates for native cancellation, accepted controlled values, and event versus layout-effect timing. That would move the distinction into parameters and conditionals rather than delete it. The current short paths make the lifecycle and ownership explicit, while `useValueChanged` reuses the same canonical hook already used by sibling controls.
