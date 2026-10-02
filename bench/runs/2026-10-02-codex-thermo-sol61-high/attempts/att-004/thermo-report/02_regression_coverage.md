# DatePicker regression coverage

## Verdict and scope

The new tests provide useful label, callback, and server-transport coverage, but they do not establish the calendar invariant that this timezone fix needs. Request changes to add deterministic checks for both representations and the initial value and bounds. Keep the existing fixtures; avoid replacing them with a custom browser or transport harness.

## Finding: Exercise initialization and selection in controlled time zones (P2)

In `tests/integration/widgets/test_datepicker.py:52–68`, the regression test checks only the value after clicking September 16 and inherits the browser’s time zone. The basic test checks the label, while the server test checks callback payload dates; neither asserts the initial displayed date or the allowable date limits. In Los Angeles the head conversion changes the Python-supplied initial value and both limits but still preserves the clicked September 16 string, so the new date assertions leave the demonstrated regression uncovered. In UTC the old and new conversion produce the same tested selection result, so that environment also fails to exercise the original UTC+ defect. Add deterministic cases in positive, negative, and zero-offset browser time zones, assert the initial date before clicking, and check both limits as well as the displayed and transported date after selection. Put the representation/time-zone matrix in focused conversion tests and retain a few browser tests for the wiring, using fixed date bounds rather than the current clock.

## What the tests actually observe

`test_basic`, at lines 42–50, creates a DatePicker with a September 20 value and September 1 minimum, but only checks that its label reads `Select date` and the console has no errors. A calendar-day error is neither a label error nor a JavaScript exception.

`test_js_on_change_executes`, at lines 52–70, opens the input, clicks a button identified by day 16, observes the callback string, and reads the displayed value after selection. That is useful coverage of the original symptom when executed in a positive-offset browser. It does not inspect the displayed September 20 initial value, the minimum, or the maximum. The day selector remains an existing implementation-specific Selenium interaction, not a new architecture finding.

`test_server_on_change_round_trip`, at lines 72–98, adds a plot and custom action so it can observe the Python callback’s old and new dates through a data source. It checks those callback data values as UTC timestamps. It does not inspect the DatePicker display after the server interaction or observe whether September 1 and the maximum day remain selectable. The plot is the established test-recording harness also used in `tests/integration/widgets/test_radio_button_group.py:43–74`; it is not evidence of an unnecessary production abstraction.

All three constructors use `datetime.utcnow()` for the maximum, at lines 43, 53, and 78. That supplies a moving datetime and makes the maximum’s time-of-day part of this already ambiguous conversion path. A fixed midnight date makes the bounds invariant easier to specify and reproduce. This is a supporting remediation for the coverage finding, not a separate cosmetic finding.

## Fixture evidence

`bokeh/_testing/plugins/selenium.py:54–81` starts a session-scoped driver. Its Chrome configuration chooses headless mode, sandbox behavior, and window size; the other supported drivers likewise receive no controlled timezone configuration in that fixture. Neither the new tests nor this driver fixture specify a timezone matrix. An externally configured CI environment may provide a particular zone, but the new tests do not require positive and negative offsets.

`bokeh/_testing/plugins/bokeh.py:174–178` waits for the recording element to become stale before returning results. The custom-action recording pattern is existing infrastructure. This review does not invent an asynchronous race finding from a lack of browser execution or require replacement of the page fixtures.

`bokehjs/test/unit.ts` and `bokehjs/test/models/index.ts` provide the existing JavaScript test entry points. The Python property tests in `bokeh/core/property/tests/test_datetime.py` exercise Python `Date` behavior but do not cover the DatePicker JavaScript conversion boundary. Keep timezone interpretation tests with the widget’s conversion logic, rather than testing a copied formula disconnected from production code.

## Verification and counterexamples

The offline probe described in `01_date_conversion.md` supplies the relevant counterexamples. In UTC, the base and head both return September 16 for the selected local string. In Los Angeles they also agree on that selected string, while the head changes September 20 to September 19 for the Python numeric value. The basic test never compares that initial display with September 20.

In Paris, the base returns September 15 for the local September 16 selection string and the head returns September 16. This establishes why a positive-offset case is needed in addition to the negative-offset regression case. Kolkata supplies a fractional-hour offset and Kiritimati a large positive offset. The probe’s extra cases cover March and November US transition dates, March and October European transition dates, December 31, and February 29. They support the worked boundary proposal, not a claim that Selenium integration tests have passed.

Verification is source inspection plus offline Node evaluation. The browser suite, server round trip, and project build were unavailable and were not run. Assertions about what each test observes are based on its source, not on a fabricated test run. The evidence demonstrates a coverage gap; it does not establish that every integration test would pass in every environment.

## Worked code-judo proposal

Separate the calendar invariant from browser orchestration. Reuse the production boundary helper from the conversion report in the JavaScript test suite, and parameterize expected calendar components over the actual supported representations. A useful case table is:

| Representation | Example | Expected local calendar components |
| --- | --- | --- |
| Python numeric value | `Date.UTC(2019, 8, 20)` | `[2019, 8, 20]` |
| Python numeric minimum | `Date.UTC(2019, 8, 1)` | `[2019, 8, 1]` |
| Python numeric maximum | `Date.UTC(2019, 8, 30)` | `[2019, 8, 30]` |
| Local selected date string | `Mon Sep 16 2019` | `[2019, 8, 16]` |
| Bare ISO date | `2019-09-20` | `[2019, 8, 20]` |

Run these cases in separate test processes or browser sessions configured for UTC, Paris, and Los Angeles. The timezone must be established before the session-scoped browser starts; setting the Python process timezone after driver startup is insufficient evidence that browser date interpretation changed. Record or assert the actual browser zone/offset for the date under test. Fractional offsets and DST/year/leap boundaries can remain cheap conversion cases, rather than multiplying every server integration test.

Use fixed `datetime(2019, 9, 1)` and `datetime(2019, 9, 30)` bounds for the browser case. Before any click, assert that the scoped input displays September 20. Verify that September 1 and September 30 are allowed while dates outside the interval are unavailable. Select September 16 once, then assert the callback value and the displayed value. Retain the existing server recording pattern and additionally observe the DatePicker input after the relevant round trip when testing that path.

This decomposition removes the need to use a complete plot/server interaction to test every arithmetic case. It gives conversion failures precise representation and timezone diagnostics, while a smaller set of browser cases proves the helper is wired into initialization, selection, and limits. The existing three tests remain valuable; their small amount of repeated widget setup does not justify a new generic fixture abstraction by itself.

## Actionable remediation

Add the focused conversion matrix with controlled timezone execution, then strengthen the existing browser test to assert initialization and limits before selection. Use the same fixed bounds in the server case. Run the permitted offline matrix immediately and the normal TypeScript and Selenium checks in a capable environment before declaring the implementation verified end to end. The present review does not modify the checkout or supply an unexecuted test patch.
