# DatePicker regression coverage

## Judgment

The new tests add useful widget smoke, JavaScript callback, and server round-trip checks. They do not establish the calendar boundary contract affected by this change. The initial-display assertion is missing, the bounds are never exercised, and the timezone is inherited from the machine. Request deterministic date conversion coverage and specific display and endpoint assertions alongside the boundary repair.

This finding is separate from the conversion bug: correcting the helper alone still leaves the original positive-offset failure dependent on where the integration tests happen to run. The remedy is a small contract matrix plus focused UI checks, not a general rewrite of the test framework.

## Source evidence

`tests/integration/widgets/test_datepicker.py:42–50` initializes a September 20 value and September 1 minimum, but checks only that `.foo label` reads `Select date` and that no console errors occur. A September 19 input display satisfies both assertions. This is the most direct missing assertion for the reproduced negative-offset regression.

The JavaScript callback test at lines 52–70 opens the picker, selects September 16, and checks the callback string and the resulting input. That checks the local selection representation. It never reads the initial September 20 input. September 16 remains an interior day even when the September 1 minimum shifts to August 31. None of these operations directly tests a shifted bound.

The server test at lines 72–98 checks the callback's old and new dates through a ColumnDataSource and CustomAction. Those values describe the model, not the initial input's displayed calendar day or the picker endpoints. `render()` passes newly created Date objects to the conversion helper, so the offset mutation does not itself change the stored original model value. The old-value assertion cannot substitute for an initial-display assertion. Whether additional Pikaday events might indirectly affect these assertions requires the unavailable browser run; the report does not claim all three tests pass in a negative-offset browser.

Each test sets `max_date=datetime.utcnow()`. This gives an otherwise fixed September 2019 scenario a bound that depends on execution time, while none of the tests inspects that bound. A fixed date in the same month makes endpoint cases deterministic and keeps the intended contract visible.

The driver fixture in `bokeh/_testing/plugins/selenium.py:56–77` configures headless Chrome or Firefox, or Safari, without a timezone override. The new test file also sets no timezone. The callback string path has no UTC day mismatch when the browser runs in UTC, as the offline base/head comparison demonstrates. Thus the original issue is not guaranteed to be exercised.

The browser result collection is already handled by `_BokehModelPage.results` in `bokeh/_testing/plugins/bokeh.py:175–178`, which waits for the test marker to become stale. The round-trip setup follows the existing radio button integration pattern. Neither duplicated fixture orchestration nor speculative race conditions justify an additional finding here.

## Measurements and commands

Read the numbered test with `nl -ba tests/integration/widgets/test_datepicker.py`, compared its structure with `tests/integration/widgets/test_radio_button_group.py`, and read the result and driver fixtures. A scoped search for `TZ` and timezone configuration in the relevant plugin and widget-test sources found no configuration used by this file.

The file is 98 lines long and contains three tests. Their asserted outcomes are label text, absence of console errors, a post-selection callback string, a post-selection input string, and two server callback calendar dates. There is no assertion for the initial input, either endpoint, or the browser timezone. No file-size decomposition is warranted. Repeated one-line DatePicker construction is not treated as an independent abstraction problem.

The offline harness described in `01_datepicker_boundary.md` provides the concrete reason for expanding the contract coverage. In UTC, both helper versions return the expected dates for every exercised representation. In Paris, the base helper returns September 19 for a local September 20 selection string, and the head repairs it. In Los Angeles, the base preserves a September 20 UTC wire value, and the head returns September 19. A test matrix that includes both sources of values and both signs of timezone offset distinguishes these behaviors reliably.

`git diff --check main...review-head` passed. The Selenium tests, Python project tests, BokehJS test runner, and browser were not run; this target permits only the focused offline Node execution used for the date conversion checks.

## Worked test proposal

Keep most conversion checks in a focused, table-driven test of the widget's raw-value boundary. Inputs should come from the documented representations, and assertions should use the requested local calendar day rather than duplicate the implementation's timezone arithmetic. For example, after implementing `_picker_date()`:

```ts
const cases = [
  {value: 1568937600000, expected: [2019, 9, 20]},
  {value: "Fri Sep 20 2019", expected: [2019, 9, 20]},
  {value: "2019-09-20", expected: [2019, 9, 20]},
  {value: 1567296000000, expected: [2019, 9, 1]},
  {value: 1569801600000, expected: [2019, 9, 30]},
]

for (const {value, expected} of cases) {
  const date = view._picker_date(value)
  expect([date.getFullYear(), date.getMonth() + 1, date.getDate()])
    .to.deep.equal(expected)
}
```

Run the same cases with a browser or isolated runtime actually configured for UTC, Europe/Paris, and America/Los_Angeles. Assert or log the effective timezone/offset for the fixed test dates to confirm the configuration takes effect. Include a fractional offset and transition-day dates if that can use the same small matrix, as the scratch reproduction already does. A Python process timezone setting alone should not be assumed to change an already running browser's timezone.

Then give the existing basic test a fixed September 30 maximum and assert the initial input before any click:

```python
el = page.driver.find_element_by_css_selector('.foo input')
assert el.get_attribute('value') == 'Fri Sep 20 2019'
```

In the calendar, verify September 1 and September 30 are selectable, while the adjacent dates are disabled or cannot be selected. Specify the full target date, including month and year where the picker exposes them, so adjacent-month cells cannot satisfy a day-only selector accidentally. Exercise a server-side date update and verify its displayed day as a wire-date re-entry check. Preserve the existing assertion that a user-selected September 16 produces `Mon Sep 16 2019` in the JavaScript callback.

This proposal splits verification according to the product contract: a compact matrix covers the date interpretation, and a few integration assertions establish that rendering, bounds, callbacks, and updates use it correctly. It avoids growing the CustomAction/plot apparatus for every timezone or date combination. The TypeScript sketch is illustrative and was not compiled; the equivalent scratch matrix was executed successfully for the candidate boundary conversion.

## Verification status and remediation

The coverage gap is established by direct inspection of every assertion in the added file and the relevant driver configuration. The base/head date behavior is reproduced offline. No claim is made that the integration suite was executed or that its full event ordering is known.

Add the initial-input assertion and fixed endpoint cases, add the representation/timezone matrix, and run the widget integration tests with controlled browser timezones in an environment that supplies the missing browser and dependencies. Keep the current callback and server round-trip tests because they cover distinct observable behavior. This focused extension is sufficient; no generic timezone abstraction or broader fixture rewrite is required for this small PR.
