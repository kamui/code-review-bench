# 02: Tests (`packages/react/src/field/control/FieldControl.test.tsx`)

## 1. Harness change (positive)

The old "avoids rerendering for uncontrolled input changes" counted renders of a wrapper component, which cannot re-render because of the control's internal state. Counting via the `render` prop measures the control itself. Good change, and it is a prerequisite for the new controlled render-count test.

## 2. Render-count test naming versus the PR's own numbers

The test "renders once per keystroke for controlled input changes" uses default validation mode and asserts `settled + 2` after two more keystrokes. The PR body says `onChange` mode costs two renders per keystroke. Suggest a second test for `validationMode="onChange"` asserting `settled + 4` for two keystrokes, so the accepted cost is pinned and a further regression is visible, or rename the current test to say "in the default validation mode".

## 3. Missing coverage

Not tested, although the PR body claims the behavior:
- Array-valued controlled `value` returning to its initial value (dirty clears).
- A controlled value the consumer rejects or rewrites does not reach field state (the reason for the early return in `onChange`).
- A pending async validator for the old value not publishing after a programmatic change.
- `clearErrors` and resolved error state after a programmatic change with server errors from `<Form>`.
- Controlled counterparts of "does not clear errors or validate when change is prevented" (currently fails to hold; see `01_field-control.md` section 2) and of the cancel test (see section 3).
- The cancel test asserts only that `validate` is not called; it does not assert `data-dirty` or `data-filled`, which is where the uncontrolled cancel behavior is inconsistent.

## Verification status

The existing suite passes locally (27 tests in this file, 1 skipped). Gaps were established by reading the test file against the PR body.
