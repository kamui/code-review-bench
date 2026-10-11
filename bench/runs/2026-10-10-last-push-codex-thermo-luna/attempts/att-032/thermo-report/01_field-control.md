# Field.Control value synchronization and lifecycle

## Scope and measurements

Reviewed the committed range `main...review-head` for `packages/react/src/field/control/FieldControl.tsx` and its test file. `FieldControl.tsx` is 207 lines after the change (the test file is 273 lines); neither crosses the skill’s 1,000-line decomposition threshold. The production diff adds the existing `useValueChanged` hook, passes a serialized controlled value into field registration, and separates controlled prop synchronization from the uncontrolled DOM change path. The new state synchronization is direct and uses the established field validation machinery rather than introducing another abstraction.

## Finding: focused state is not cleared on unmount

`FieldControl.tsx:158-164` sets the root’s focused state on focus and clears it only in the `onBlur` handler. The component does not register an effect cleanup that calls `setFocused(false)`. Removing the focused DOM input does not guarantee a blur event, so the root can retain `focused === true`; the field root then continues projecting `data-focused` to itself and its descendants. The PR body explicitly says that a control unmounting while focused no longer latches `data-focused`, but this code does not implement that behavior. This is a lifecycle gap, not a general request to alter focus semantics.

### Worked code-judo proposal

Treat focus as state owned by a mounted control. Track whether this control currently owns focus, set ownership in `onFocus`, and release it both in `onBlur` and in an effect cleanup. If multiple controls can participate in a field, make release conditional on the releasing control still being the active owner; otherwise a replaced control’s cleanup can clear focus established by its replacement. An explicit active-control ref in the field root is the narrowest shared ownership boundary if that multi-control case exists. This removes the dependency on a browser-generated blur during DOM removal and keeps focus transitions tied to the control lifecycle.

Add a regression test that focuses the control, removes it from the tree, and asserts the root no longer exposes `data-focused`. If the PR’s disabled-state claim is intended to include a control remaining mounted but becoming disabled, test that transition separately and define whether it clears focus immediately.

## Verification status

Executed from the clone root:

```sh
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx
```

Result: one test file passed; 26 tests passed and 1 was skipped. `git diff --check main...review-head` produced no output. The existing suite does not cover focused-control unmount. Browser execution was unavailable under the run policy, so the finding is based on the source lifecycle path rather than rendered browser output.

## Other structural review notes

The controlled and uncontrolled branches have different necessary scheduling constraints: uncontrolled input updates must keep avoiding a React rerender on every keystroke, while controlled state arrives through the prop. The new hook keeps the latter on the repository’s established value-change path and the focused render-count tests exercise the intended controlled echo. Given the small file size and reuse of existing hooks, I found no higher-confidence structural simplification that warrants a separate finding.
