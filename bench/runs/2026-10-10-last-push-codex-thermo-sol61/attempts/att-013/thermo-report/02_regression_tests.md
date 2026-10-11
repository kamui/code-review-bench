# Regression coverage

## Finding and disposition

The added tests need to observe the date before user selection and verify the timezone in which they exercise it. The actionable anchor is `tests/integration/widgets/test_datepicker.py:42–48`: `test_basic` constructs a DatePicker with a September 20 value but checks only the label. This omission hides the concrete initialization regression reported in the date-conversion detail, not merely a hypothetical need for more tests.

## Assertion and fixture evidence

`test_basic` at lines 42–50 checks the label and console errors. It never reads the initial input value or inspects the calendar. An incorrectly selected date is valid JavaScript and need not emit a console error.

`test_js_on_change_executes` at lines 52–70 opens the calendar and clicks day 16. It then checks the callback string and displayed input against September 16. That covers the original symptom if the browser happens to run in an affected timezone. It does not check what was displayed before the click. The scratch probe confirms that in Los Angeles a serialized September 20 initializes as September 19 at head, while a local September 16 string converts to September 16. Both dates remain in September, and day 16 remains inside the shifted bounds, so the chosen action does not expose the bad initial state.

`test_server_on_change_round_trip` at lines 72–98 records old/new server-side dates via a data source and a CustomAction, then asserts those dates. That useful transport check does not read the initial or final input display and does not verify boundary availability. The underlying model can retain the correct timestamp while the widget receives an incorrect Date for rendering; model-only assertions cannot establish display correctness. The recording pattern follows `tests/integration/widgets/test_radio_button_group.py`, so its plotting scaffolding is not an independent architectural regression.

All three constructors use `datetime.utcnow()` for the maximum. This adds wall-clock dependency to otherwise fixed September 2019 examples. A fixed September 30 maximum is enough for the date and callback scenarios and gives an exact inclusive-bound expectation.

The driver fixture in `bokeh/_testing/plugins/selenium.py:54–81` has session scope and creates Chrome, Firefox, or Safari without a timezone override or an assertion of the effective timezone. Neither the added test file nor the inspected driver fixture controls `TZ`. The added test file therefore depends on the surrounding runner's ambient timezone. No claim is made about the historical CI timezone: it was not available for execution in this task.

In the focused UTC process, both committed helper versions have zero mismatches in all 25 conversion cases, including the test's September 16 selection. That shows precisely why successful tests in UTC would not distinguish the original broken selection behavior from the proposed fix. In Paris the original helper returns September 15 for a local September 16 selection, and the head returns September 16. In Los Angeles the head's initial value and bounds are wrong, although post-selection conversion is correct.

## Worked test restructuring

Keep the current JavaScript callback and server transport tests. Add assertions of the initial input value to the basic test, before opening or changing the picker. A minimal worked change is:

```python
dp = DatePicker(
    title='Select date',
    value=datetime(2019, 9, 20),
    min_date=datetime(2019, 9, 1),
    max_date=datetime(2019, 9, 30),
    css_classes=["foo"],
)
page = bokeh_model_page(dp)
assert page.driver.find_element_by_css_selector('.foo label').text == 'Select date'
assert page.driver.find_element_by_css_selector('.foo input').get_attribute('value') == 'Fri Sep 20 2019'
```

Use those same fixed date inputs in the other tests. This expresses the calendar fixture once in concept; a small widget factory can remove the three long constructor copies if the scenarios grow. Do not introduce a general widget-testing framework or spread timezone conditionals through each interaction test.

Add a focused limit scenario that opens the calendar and confirms September 1 and September 30 are selectable, while August 31 and October 1 are excluded. Scope selectors to the intended widget and the requested calendar date; when examining days from adjacent months, use month/year attributes in addition to `data-pika-day` so duplicate day numbers do not make the assertion ambiguous. Navigate months as needed. After a click on an inclusive limit, check both the model callback result and the input date. The head's September 29 maximum in Los Angeles should fail the September 30 assertion, and its August 31 minimum should fail the exclusion assertion.

Run these date scenarios in UTC, Europe/Paris, and America/Los_Angeles. A reliable existing-session approach is separate test invocations with each browser process launched in the requested environment. Setting a timezone environment variable inside an individual test after the session-scoped driver has launched is insufficient. Verify the browser's actual September 20 offset with `new Date(2019, 8, 20).getTimezoneOffset()`; expected values are 0, -120, and 420 minutes for those three environments. Browser support for process-level `TZ` must itself be checked; if a supported browser needs its own timezone mechanism, configure it at driver startup. This report does not claim that a Node `TZ` override configures Selenium.

For fast unit-level protection of the conversion boundary, use a compact input table covering a serialized UTC-midnight number, a browser-local selection string, an ISO date-only string if that form remains supported, and a non-midnight numeric datetime. Check requested local year/month/day, and use fixed min/max cases. The [scratch proposal probe](../date-picker-probe.js) is a worked example of that table across eight timezones, not a new committed project test or a replacement for browser assertions. It reduces test complexity by sharing input cases instead of copying server plots per timezone.

The critical before/after behavior is explicit: on the current head, the initial input assertion should fail in Los Angeles while post-selection assertions can pass; on the base, selection display should fail in Paris; with the corrected boundary, both should pass, including inclusive limits. This distinguishes the original defect and the introduced regression in opposite timezone directions.

## Verification status and limits

Verified by static inspection: what each new test asserts, fixed dates and moving maximum, driver session scope, absence of timezone control in the changed tests and inspected driver fixture, and use of the existing server-recording pattern. Verified by offline execution of the exact helpers: the contrasting initial/selection dates and identical base/head outputs in UTC. Measurements: the new test module is 98 lines and does not cross the 1,000-line threshold; the committed diff passes `git diff --check`.

Not executed: the integration tests, browser effective timezone, enabled/disabled date buttons, server callback transport, or proposed test snippets. Browser, Selenium, project build, and dependency installation were unavailable. Source inspection and Node checks establish a coverage gap and a conversion regression; they do not establish the historical pass/fail status of the full suite. No checkout files were changed.
