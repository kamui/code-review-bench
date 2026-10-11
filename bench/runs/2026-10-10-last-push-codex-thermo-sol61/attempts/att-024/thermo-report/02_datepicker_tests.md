# DatePicker regression coverage

## Judgment and source evidence

F2 is a P2 coverage gap in the newly added tests, anchored at `tests/integration/widgets/test_datepicker.py:42–50`. The suite adds useful label, selection callback, and server callback coverage, but its assertions do not establish the date conversion contract that this PR changes. The distinct corrective action is to make the new regression coverage detect the failing initial-value and date-limit paths as well as the reported selection path.

`test_basic` supplies September 20, 2019 as the initial value, September 1 as the minimum, and `datetime.utcnow()` as the maximum. At lines 47–50 it checks only `Select date` and the absence of console errors. An input displaying September 19 satisfies those assertions just as an input displaying September 20 does.

`test_js_on_change_executes` selects September 16, an interior day, and checks both `cb_obj.value` and the displayed value against `Mon Sep 16 2019` at lines 64–68. This is useful coverage for a locally generated selection string. It does not inspect the numeric initial value first, and the chosen day remains inside the allowable interval even when a UTC− conversion shifts both limits backward.

`test_server_on_change_round_trip` records the server callback's old/new values through a `ColumnDataSource` and toolbar action. It asserts their UTC calendar days at lines 94–98, but never checks the DatePicker input after a server assignment. The callback only writes the source's `val` list, so the test does not actually perform a numeric server-originated update of `dp.value`.

The shared driver fixture at `bokeh/_testing/plugins/selenium.py:54–84` configures headless browsers and a window size but no timezone. None of the new tests changes that. The review packet contains no executed CI timezone guarantee, so this review cannot assume either a positive or negative browser offset.

At UTC the base and head conversion are identical for these cases. In UTC− zones the head fixes no selection-string problem because the base already handles that path; instead, the regression occurs before clicking. Thus these tests can pass while the new numeric conversion is wrong. This is a source-level assessment of assertion coverage, not a claim that Selenium was run here.

## Inspection and executed evidence

The relevant source was inspected with:

```sh
git diff main...review-head -- tests/integration/widgets/test_datepicker.py
nl -ba tests/integration/widgets/test_datepicker.py
cat tests/integration/widgets/test_radio_button_group.py
sed -n '1,230p' bokeh/_testing/plugins/bokeh.py
sed -n '30,125p' bokeh/_testing/plugins/selenium.py
```

The existing radio-button integration test uses the same source/plot/toolbar technique to observe a Python callback. The DatePicker version follows an established pattern; the callback probe is not itself an abstraction regression. The new test file contains 98 lines, so there is no giant-file finding.

The scratch probe described in `01_date_conversion.md` supplies the exact numeric initial date used in these tests. In Los Angeles and Honolulu it produces September 19 under the head and September 20 under the base. The same probe shows that `Mon Sep 16 2019` still renders September 16 under both implementations in those zones. That is the concrete combination the current assertions overlook.

The probe verifies seven calendar cases per zone: numeric initial value, numeric minimum, numeric maximum, browser selection string, date-only ISO string, winter numeric value, and winter selection string. It also exercises the same selected calendar day represented first as a local string and then as a UTC number. The proposed boundary converter satisfies all 35 calendar expectations and both representations of the selected day across five zones.

This is meaningful behavioral verification of the changed conversion, but it is not a project test. The script copies the conversion bodies rather than importing a built widget; no DOM, Pikaday, CustomJS, Python process, or server is involved. The new Selenium suite and the build were unavailable under the task policy. There is no claim that a complete browser test passed or failed.

## Worked test design

Put the conversion cases around the actual converter used by the widget once the implementation fix is made. The converter should remain small and pure so the cases establish its representation contract without a rendered plot or a server. Keep test data fixed rather than mixing a 2019 fixture with the execution day's maximum.

The minimal conversion matrix has these expectations in every timezone:

| Representation | Input | Expected local calendar fields |
| --- | --- | --- |
| Python initial date | `Date.UTC(2019, 8, 20)` | `[2019, 9, 20]` |
| Python minimum date | `Date.UTC(2019, 8, 1)` | `[2019, 9, 1]` |
| Python maximum date | `Date.UTC(2019, 8, 30)` | `[2019, 9, 30]` |
| Browser selection | `new Date(2019, 8, 16).toDateString()` | `[2019, 9, 16]` |
| Date-only ISO string, if retained as supported input | `2019-09-20` | `[2019, 9, 20]` |

Run the cases in fresh JavaScript processes configured for UTC, a positive-offset zone, and a negative-offset zone. Paris and Los Angeles are useful because the source defect involves a positive offset while the new regression involves a negative offset. Add winter cases for those zones to cover their alternate offsets. The review additionally checked a fractional-hour positive offset and a negative zone without daylight saving.

Here is the essential assertion shape, already exercised in the scratch proposal probe:

```js
const result = date_from_value(rawValue)
assert.deepEqual(
  [result.getFullYear(), result.getMonth() + 1, result.getDate()],
  expectedCalendarFields,
)
```

Assert calendar fields rather than the returned date's UTC ISO day. The output is deliberately a local Date for Pikaday, so an ISO assertion could recreate the representation confusion that caused the defect. Arrange timezone configuration before the JavaScript runtime starts; configure the browser process itself for browser checks. Altering only a Python server process's timezone is insufficient.

Retain a focused browser scenario for widget wiring. Create a picker with initial September 20, minimum September 1, and maximum September 30. Before clicking anything, assert that its input displays September 20. Inspect the calendar across the month boundaries and verify August 31 and October 1 cannot be selected while September 1 and September 30 can be selected. Qualify calendar selectors with the year and month if the rendered calendar exposes duplicate day numbers. The exact disabled-day selectors require the project's installed Pikaday and should be determined in that environment rather than guessed in this report.

Then select September 16 and retain the existing assertions on the callback string and displayed date. This checks that the positive-zone fix survives while the callback API remains stable. Inspect the input with the widget's `.foo` scope when multiple widgets are present.

For server coverage, use a deliberate server callback or update action to assign another date to `dp.value`, for example September 21. Wait for the existing fixture's server-to-browser observation mechanism or a condition on the input, then assert the rendered input displays September 21. Reading the callback's old/new values alone does not exercise that update. This separates the two representations through observable behavior without depending on an undocumented assumption that every browser-originated update will be echoed.

The existing three repetitions of the picker constructor do not justify a standalone duplication finding. If the expanded matrix makes setup cumbersome, use one small test-local factory with fixed defaults and explicit overrides. Avoid a generalized widget testing framework: the important simplification is to put the conversion truth table next to the pure converter and keep browser coverage focused on integration.

## Required follow-through and limits

The converter tests should fail for the base's Paris selection-string case and the head's Los Angeles numeric case, then pass for the corrected boundary. The browser checks should separately exercise initial display, bounds, selection, and a server-originated date update. This makes it difficult to fix one side of the representation contract while silently breaking the other.

The exact browser interaction and timezone configuration were not executed in this environment. Selenium, a browser, dependency installation, and the project build were unavailable. The five scratch Node commands succeeded offline and establish only the conversion behavior summarized above. No checkout file was changed and no network was used.
