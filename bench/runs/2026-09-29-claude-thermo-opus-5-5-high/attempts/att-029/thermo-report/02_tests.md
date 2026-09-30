# 02 — `FieldControl.test.tsx` changes (detail)

Scope: `packages/react/src/field/control/FieldControl.test.tsx` (+149 / −6) at `review-head`.

Run: `TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx`
reports 26 passed and 1 skipped (the skipped test is the pre-existing chromium-only SSR `autoFocus` case).

## What improved

The uncontrolled render-count test ("avoids rerendering for uncontrolled input changes") used to count renders of a wrapper
component, `RenderCountedControl`. That wrapper does not re-render when `Field.Control` re-renders from field context changes, so the
old test could not detect the regression it was named after. The PR moves the counter into the `render` prop, which runs every time
`Field.Control` renders. The test now measures the component it describes, which is a real improvement.

The new tests are behavioural and pin the fixes described in the PR body:

- numeric dirty clears when the value returns to its initial value
- programmatic changes sync filled, dirty, and validation
- an empty controlled value is not filled on mount
- canceling a change stops validation
- a controlled user change validates exactly once

## Finding 4 — The controlled render-count test name and comment overclaim what it pins (legibility; verified by measurement)

The new test "renders once per keystroke for controlled input changes" runs in the default `onSubmit` validation mode. Its
comment says "The controlled echo must not schedule a second render per keystroke." The PR body's own performance table, and the
scratch measurement in `01_field-control.md` (`renders/keystroke onChange [2, 2, 2, 2]`), both show that in `onChange` mode the
controlled path *does* take a second render on every keystroke, by design. The name and comment read as a general guarantee, but
the test only holds for one validation mode. A future contributor who sees two renders per keystroke in `onChange` mode could
reasonably think this invariant has been broken, or could "fix" it by moving work back into the handler, which would reintroduce
the second sync path. Remedy: rename it to something like "renders once per keystroke for controlled input changes when
validating on submit", and change the comment to say the extra `onChange`-mode render is expected (the cost of
`useValueChanged`). Optionally, add an `onChange`-mode assertion of exactly two renders per keystroke so that cost is pinned
instead of just tolerated.

## Coverage gap tied to Finding 3

"does not clear errors or validate when change is prevented" covers only the uncontrolled mode. After the PR, controlled and
uncontrolled modes handle a prevented native `input` event differently (see Finding 3 in `01_field-control.md`). Add a controlled
twin so the chosen controlled behaviour is pinned explicitly.
