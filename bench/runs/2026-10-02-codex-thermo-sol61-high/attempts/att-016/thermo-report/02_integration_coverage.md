# DatePicker integration coverage

## Judgment

This subsystem supports F1 in [the summary](summary.md). It does not introduce a second finding. The tests exercise useful widget interactions but omit the representation transition that causes the confirmed regression: the initial Python numeric date versus the local string after a browser selection.

The new file is 98 lines, contains three tests, and introduces no branching complexity or new test infrastructure. Its server callback scaffolding follows the neighboring radio-button and color-picker tests. Extracting that scaffolding solely for this PR would add an abstraction without solving the date boundary.

## Source evidence

In `tests/integration/widgets/test_datepicker.py:42–50`, `test_basic` constructs a September 20 value with a September 1 minimum, but checks only the label and console errors. It never inspects the input value that would reveal the September 19 initial display in western time zones.

At lines 52–70, `test_js_on_change_executes` clicks September 16 and then checks both the callback string and the input string. This is relevant to the originating issue: a selected local day must survive the model-change rerender. However, after that click `_on_select` has replaced the numeric model value with a local `toDateString()` value. The new helper correctly converts that representation in Los Angeles as well as Paris. This test therefore does not inspect the representation that regressed.

At lines 72–98, `test_server_on_change_round_trip` records Python callback `old` and `new` dates through a data source and a custom action. Its assertions use `utcfromtimestamp` to check the two serialized data values, which verifies callback date semantics rather than the widget's displayed date. The Python model initially retains September 20 even when the browser adapter supplies September 19 to Pikaday. The old-value assertion cannot establish that the initial display was correct.

All three constructors set limits, but no test selects or inspects a limit or a day immediately outside it. The September 16 click lies inside both the intended interval and the interval shifted by the defective helper. All three tests also use `datetime.utcnow()` for their upper bound; a fixed bound would make exact boundary checks straightforward.

The browser fixture in `bokeh/_testing/plugins/selenium.py:59–79` configures a session driver and basic window options. It does not configure a timezone matrix. A UTC host exercises neither the original local-selection defect nor F1. Running in a western timezone still needs an initial-value or exact-limit assertion to expose F1; merely repeating the current selection test there is insufficient.

## Commands and review measurements

The test and fixture evidence was read with:

```sh
nl -ba tests/integration/widgets/test_datepicker.py
nl -ba bokeh/_testing/plugins/selenium.py
sed -n '1,340p' bokeh/_testing/plugins/bokeh.py
cat tests/integration/widgets/test_radio_button_group.py
cat tests/integration/widgets/test_color_picker.py
cat bokehjs/test/models/widgets/index.ts
cat bokehjs/test/models/widgets/multiselect.ts
```

The model-page result fixture waits for the test-record element to become stale before reading recorded JavaScript results. The server-page fixture shares the custom-action testing mechanism used by existing widget tests. No new synchronization or callback-delivery defect was established, and no browser execution claim is made.

`bokehjs/test/models/widgets/index.ts` imports the existing widget test modules but has no DatePicker test. Focused coverage for the converter can be added through the existing TypeScript test organization rather than creating a parallel permanent scratch harness. The review harness demonstrates the important test cases but is not a substitute for registered project tests.

## Worked test simplification

Keep each testing layer responsible for the behavior it can observe directly. The pure adapter should carry the detailed representation/timezone matrix, and Selenium should carry a small set of assertions that prove the adapter is wired correctly to the displayed input and calendar limits. This avoids making every calendar arithmetic case boot a server and a plot.

For the pure converter, compare the returned local year, month, and day with the requested date. Use a UTC-midnight number, the corresponding `toDateString()` string, and an ISO date-only string. Cover UTC, Paris, and Los Angeles at minimum, with both winter and summer dates. The review additionally checked London, New York, Kolkata, Kiritimati, and the two European transition dates. Configure time zones in isolated processes or through the actual supported test/browser setup; a host-side Python timezone change alone does not prove the browser's timezone.

For the initial display, extend `test_basic` after page creation:

```python
el = page.driver.find_element_by_css_selector('.foo input')
assert el.get_attribute('value') == 'Fri Sep 20 2019'
```

Run this with an explicitly configured western browser timezone. It checks the exact initial value rather than a model field or a callback result. Retain the existing label assertion.

For limits, use fixed September 1 and September 30, 2019 values. Inspect the day buttons by full date coordinates, not the day number alone, because adjacent months can show the same day number. Check that September 1 and September 30 are enabled, and August 31 and October 1 are unavailable. A western timezone then reveals both an erroneously relaxed minimum and an erroneously tightened maximum. Use the actual pinned Pikaday button attributes and disabled-state representation when implementing these checks; that dependency's source and a browser are unavailable for this review.

Keep the existing September 16 callback and displayed-value assertions, and run them in an eastern browser timezone to cover the originating selection bug. For the server case, inspect the displayed input after the round trip as well as the callback dates, and use the existing fixture's result synchronization. Do not change callback strings simply to simplify assertions.

If shared setup is needed while adding these cases, extract only the fixed DatePicker construction or a small full-date calendar selector once real repetition arises. The current three constructors and familiar server scaffolding do not justify a generic widget test framework.

## Verification status

All test and fixture conclusions above are source inspection. No Python test execution, Selenium, browser, fixture server, npm installation, or build was performed because those are unavailable or outside the target's focused execution allowance.

The only executable verification was the offline Node matrix described in [the conversion report](01_date_conversion.md). Its proposed adapter passes all 126 cases, but project compilation and browser/server integration remain required before accepting a remedy.

## Checkout integrity

The review began with a clean `git status --short`, the pinned head `36549bca3a63d581f7b68d08054a7813c1e6a499`, and tracked-index listing SHA-256 `58a9e2bd655d0210956008c65096fb4c12a0e1ba9e1d0e7e9d4f09eeb694d7d3`. Reports, the index, the harness, and results were written only under `clone-work`. Final checks confirmed the same pinned head, clean status, and tracked-index identity. No remedies were applied.
