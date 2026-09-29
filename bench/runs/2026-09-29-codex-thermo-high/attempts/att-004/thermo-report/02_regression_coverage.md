# Regression coverage

## Finding

`tests/integration/widgets/test_datepicker.py:42-70` checks the label, then selects September 16 and checks the resulting model/input values; the server test likewise checks selected old/new values. None asserts the initial display of September 20 or a model-driven rerender from a local `toDateString()` value. These checks can pass before the patch and do not detect either the UTC+ regression being fixed or the UTC− timestamp regression introduced by the offset adjustment. Add a focused initial-display/rerender regression that runs with an explicit UTC+ timezone and a UTC− timestamp case, and assert both the rendered day and model value.

## Evidence and analysis

`test_basic` renders a DatePicker with a September 20 value, but it only asserts the title text and absence of console errors. `test_js_on_change_executes` selects September 16 and asserts the callback value and input value after selection. The server round-trip test similarly chooses September 16, then checks the callback’s old and new timestamp values. These assertions validate selection/event behavior, not how `defaultDate` is derived during initial render or a render triggered by a changed model value.

The production change is inside `_unlocal_date`, which is called to construct `defaultDate`, `minDate`, and `maxDate` during render. The input display is initialized by Pikaday from this configuration. A regression test should therefore assert the actual input and/or selected calendar day immediately after rendering the desired value. To reproduce the reported case, it should also trigger a model change that stores a date string, since `_on_select` writes `date.toDateString()` and a subsequent render parses that as a local date. The new UTC− case should use a UTC-midnight timestamp and assert that it remains on the UTC date.

## Code-judo proposal

Factor the test’s observable requirement into one compact regression focused on calendar-date normalization: start with a known model date, assert the initial input value, cause a model update/re-render using the second accepted representation, and assert the date again. Parameterize or otherwise run the case with controlled timezone and input representation so a single test structure catches both the original UTC+ bug and the UTC− regression. Keep selection/server round-trip tests for their distinct event contract rather than using their assertions as proxies for display normalization.

## Verification

The new assertions are absent from the added integration file. This is a source review; Selenium and a browser are unavailable per the packet, so the suite was not run. The scratch Node reproduction in the date-normalization detail confirms that the two representations produce different outcomes in the current helper.

## Action

Add tests that fail against the merge-base implementation for the reported UTC+ local-string display case and fail against this PR for a UTC− UTC-timestamp display case. Verify the rendered calendar day at initial render and after model-driven rerender, then run the focused Selenium tests in an available browser environment.
