# DatePicker regression tests

## Judgment

The actionable test finding is the ambient-timezone dependency surrounding `tests/integration/widgets/test_datepicker.py:64–68`. A timezone correction needs a test environment that deliberately distinguishes UTC-encoded dates from local selection dates. The added tests provide useful callback plumbing checks, but their assertions do not establish that contract across the affected timezone directions.

## Source evidence

The entire new file contains 98 lines and three tests. `test_basic`, at lines 42–50, creates a widget with September 20 as its initial value and September 1 as its minimum, then asserts only the label and console status. It does not inspect the initially displayed date. Consequently, a widget initialized with the wrong day can pass this test.

`test_js_on_change_executes`, at lines 52–70, clicks September 16 and checks both the callback's string and the input's displayed string. The displayed check at lines 67–68 is useful for the originating defect when the browser is in UTC+, but UTC gives the same output with either helper. It never asserts the pre-click display, nor does it test either bound.

`test_server_on_change_round_trip`, at lines 72–98, records the old and new server values through a ColumnDataSource and CustomAction. It checks the dates in the returned source data. It can reveal an incorrect initial value in a UTC− environment if initialization changes that value, but it does not explicitly arrange such an environment or inspect the input after a server-driven update. This review does not assume that the server test will pass in all UTC− browsers; its execution was unavailable.

`bokeh/_testing/plugins/selenium.py:54–83` creates a session-scoped webdriver. The inspected Chrome and Firefox options select headless mode and window size; neither sets a timezone. The new test file has no timezone setup or parameterization. No changed execution configuration establishes a timezone matrix. All three fixtures use `datetime.utcnow()` for the maximum while fixing the selected dates in September 2019, which also makes the allowed interval depend on execution time rather than the datepicker contract being tested.

The existing page helper at `bokeh/_testing/plugins/bokeh.py:174–178` waits for the recording element to become stale before returning recorded results. The action/data transport follows the existing radio-button-group test pattern. There is no evidence here for an independent synchronization finding or a need to redesign that shared fixture.

## Measurements and commands

Read-only inspection included:

```sh
nl -ba tests/integration/widgets/test_datepicker.py
nl -ba tests/integration/widgets/test_radio_button_group.py
nl -ba bokeh/_testing/plugins/selenium.py
nl -ba bokeh/_testing/plugins/bokeh.py
rg -n 'pikaday|timezone|TZ|setTimezone|time_zone' bokehjs/package.json tests/integration bokeh/_testing/plugins bokehjs/src/lib/core
nl -ba bokehjs/test/models/widgets/paragraph.ts
nl -ba bokehjs/test/models/widgets/index.ts
```

The offline harness described in [01_date_boundary.md](01_date_boundary.md) independently executes both helper bodies at UTC and five non-UTC zones. At UTC the base and head agree on the initial timestamp, both bound timestamps, an ISO date-only value, and the selection string. In Paris, the local selection differentiates the broken base from head; in Los Angeles and New York, the timestamp inputs differentiate the correct base from broken head. Thus a deliberately chosen timezone matrix is necessary for these scenarios to reject both defects.

The existing JavaScript widget test index imports other widget suites but no DatePicker suite. There is a natural home for focused conversion assertions in a DatePicker widget suite, rather than duplicating date arithmetic expectations throughout several Selenium functions.

## Worked test restructuring

Keep a small deterministic conversion matrix close to the widget boundary. The expected outputs must be literal calendar dates drawn from the product contract, not calculated with the same offset operations as the implementation. For each explicitly launched timezone environment, exercise numeric Python-style values, the widget's selection strings, and supported date-only strings. Include initialization, both bound conversions, leap day, and dates near DST changes.

| Environment | Numeric initial date expected | Local selection expected | Purpose |
| --- | --- | --- | --- |
| UTC | September 20 | September 16 | Baseline contract |
| Europe/Paris | September 20 | September 16 | Reject the original selection defect |
| America/Los_Angeles | September 20 | September 16 | Reject the new timestamp regression |

The scratch harness is a reproducible proof of these checks, not a proposed replacement for the project's test infrastructure. A production widget test should call the actual conversion code instead of extracting source text. Use a focused test entry point supported by the project's runner; this review does not propose importing the entire browser-oriented widget into bare Node without its dependencies.

For browser coverage, launch a separate browser/session under each configured timezone rather than parameterizing a string inside Python while leaving the session-scoped driver untouched. Where the runner uses a local browser that honors `TZ`, start that process with the desired environment before creating the driver, and confirm the browser's offset for a fixed date. A remote browser needs its timezone configured at the browser host. This review has not verified either mechanism for the historical supported browser versions, so that setup must be checked when execution becomes available.

Use fixed fixture dates, for example a September 20 value with September 1 and September 30 bounds. In `test_basic`, assert the initial input value before opening the picker. Check that September 1 and September 30 are enabled and the adjacent August 31 and October 1 dates are disabled. The UTC− regression currently permits the day before the intended minimum and excludes the intended maximum at the conversion boundary; these assertions make the interval contract visible.

Retain the existing September 16 click and the callback-string assertion, then assert the displayed input after selection. In the server test, deliberately assign a fixed Python date from a server callback and wait for the resulting browser model/render update before reading the displayed value. This covers the transition back from a browser-local string to a server-encoded timestamp rather than testing only a click that leaves the value as a local string.

The repeated DatePicker constructors can share a small local fixed-data factory if these additional scenarios make them cumbersome. Do not introduce a general widget-test framework for three tests. The useful structural simplification is separating the representation matrix from the integration plumbing, so each date conversion edge case does not require another plot and server fixture.

## Verification status and limits

The coverage gap is verified through source inspection, and the timezone cases are verified by the permitted offline Node check. The Selenium suite, browser behavior, real timezone launch configuration, project build, and server round trip were not executed. The proposed tests are actionable remediation, not claimed passing test results.

This finding is separate from the production boundary error: fixing the arithmetic alone does not ensure that CI rejects a recurrence in either timezone direction. Establish the explicit timezone matrix and date assertions as the regression contract, then execute the browser and server checks when the environment supports them.
