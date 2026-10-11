# 02 — `tests/integration/widgets/test_datepicker.py`

Subsystem: the new Selenium integration test file (98 lines, all added by this PR).
Carries the evidence for finding 3 of `summary.md`.

## What the file contains

One class, `Test_DatePicker`, with three tests.

`test_basic` (lines 42–50) builds the widget, loads the page, and asserts the label text and the
absence of console errors.

`test_js_on_change_executes` (lines 52–70) attaches a `CustomJS` recorder, opens the picker,
clicks `button[data-pika-day="16"]`, and asserts both the recorded `cb_obj.value` and the
`.bk-input` element's value equal `'Mon Sep 16 2019'`.

`test_server_on_change_round_trip` (lines 72–98) does the same click against a Bokeh server and
asserts the `old` and `new` values seen by a Python `on_change` callback are September 20 and
September 16.

The second test's check of the input box after a click is the symptom from #9129, so the right
observation is in the file. The problem is the conditions under which it is made.

## Measurement 1 — nothing sets a timezone

The bug and the fix both depend on `Date.prototype.getTimezoneOffset()` in the browser. I looked
for anything that controls it.

```
grep -rn "TZ\b" .travis.yml scripts/ci            # no matches
grep -rn "TZ\b\|timezone" tests/integration bokeh/_testing   # no matches
```

`scripts/ci/test.integration` line 6 is `py.test -v --tb=short -m integration --driver chrome`
with no environment prefix. The `driver` fixture in `bokeh/_testing/plugins/selenium.py`
(lines 54–67) is `scope="session"` and passes only `--headless`, `--no-sandbox` and
`--window-size` to Chrome, so there is one browser for the whole run and no hook for varying its
zone per test.

The tests therefore run in whatever zone the machine happens to be in.

## Measurement 2 — in UTC the head and the merge-base are indistinguishable

When the offset is zero, the two lines the PR adds subtract zero. The UTC row of the matrix in
`01_date_picker_view.md` shows it: for both the Python-origin number and the JS-origin string, the
merge-base body and the head body return the same day.

```
UTC   py-origin Sep 20 -> old: Fri Sep 20 2019  new: Fri Sep 20 2019 | js-origin Sep 16 -> old: Mon Sep 16 2019  new: Mon Sep 16 2019
```

Every input the three tests use goes through that helper, so on a UTC machine the file passes
against the merge-base as well as the head. It would also keep passing if the two added lines
were deleted again. Hosted CI images usually default to UTC; I could not confirm the zone of this
project's CI job from the repository.

The same matrix shows what the tests would see elsewhere. In Europe/Paris the second test's
`.bk-input` assertion would fail at the merge-base (`Sun Sep 15 2019`) and pass at the head, which
is the regression check the PR wants. In America/Los_Angeles a first-render assertion would pass
at the merge-base and fail at the head (`Thu Sep 19 2019`), which is finding 1; no such assertion
exists.

## Measurement 3 — the observations that are missing

`test_basic` is the only test that looks at the page before any click, and it reads only
`.foo label`. Nothing in the file reads `.foo input` on first render, so the initial display of a
Python-supplied `value` is untested in every zone.

All three tests pass `min_date` and `max_date`, and none asserts anything about them. No test
checks that a day outside the window is disabled or that the boundary days are selectable.

`test_server_on_change_round_trip` does not read the input box after the click, although the
displayed value after a server round trip is the scenario in the issue's reproduction script.

## Measurement 4 — wall clock and repetition

`max_date=datetime.utcnow()` appears on lines 43, 53 and 78. The assertions hard-code September
2019 and the selector hard-codes day 16 of whichever month is displayed, so the tests only need a
maximum on or after 2019‑09‑20. Using the current time adds a moving input that no assertion
reads, and it is a `datetime` with a time-of-day component passed to a `Date` property, which
makes the bound's exact value depend on when the test runs.

The constructor call

```python
DatePicker(title='Select date', value=datetime(2019, 9, 20), min_date=datetime(2019, 9, 1), max_date=datetime.utcnow(), css_classes=["foo"])
```

is repeated verbatim on those same three lines, and the four-line open-and-click-16 sequence is
repeated at lines 58–62 and 86–90. Sibling files in this directory repeat their widget
construction too (`test_radio_button_group.py` does it with a module-level `LABELS` constant), so
the repetition follows local convention; it matters here mainly because fixing the bounds means
editing three identical lines.

The licence header's `2012 - 2017` matches all 21 sibling files and is not a finding.

## Worked proposal

The cheapest and strongest test is not a Selenium test. Once the conversion is the pure function
proposed in `01_date_picker_view.md`, it can be tested where the bokehjs unit tests already run:
`bokehjs/make/tasks/test.ts` runs them under mocha in node, and node honours `TZ`.
`bokehjs/test/models/widgets/` already holds `paragraph.ts`, `slider.ts` and others in the shape
below.

```ts
import {expect} from "chai"

import {to_local_day} from "@bokehjs/models/widgets/date_picker"

describe("DatePicker to_local_day", () => {
  // 2019-09-20 as the Python serializer sends it
  const from_python = Date.UTC(2019, 8, 20)
  // 2019-09-16 as _on_select() writes it
  const from_select = "Mon Sep 16 2019"

  it("should read a serialized date as the day Python sent", () => {
    expect(to_local_day(from_python).toDateString()).to.be.equal("Fri Sep 20 2019")
  })

  it("should read a selected date as the day the user clicked", () => {
    expect(to_local_day(from_select).toDateString()).to.be.equal("Mon Sep 16 2019")
  })
})
```

Run the unit task once per zone, for example with `TZ=Europe/Paris` and
`TZ=America/Los_Angeles` as well as the default. The first case fails at the head west of
Greenwich and the second fails at the merge-base east of it, so the pair pins both directions.

For the Selenium file, keep it and tighten it:

```python
def _date_picker():
    return DatePicker(title='Select date', value=date(2019, 9, 20), min_date=date(2019, 9, 1), max_date=date(2019, 9, 30), css_classes=["foo"])
```

Then have `test_basic` assert `page.driver.find_element_by_css_selector('.foo input').get_attribute('value') == 'Fri Sep 20 2019'`,
add a bounds test that asserts the September 1 and September 30 buttons are enabled, and add the
`.bk-input` read to the server round-trip test. If the integration job is to guard the timezone
behaviour by itself, invoke it a second time with `TZ` exported to a non-UTC zone; headless
Chrome on Linux takes its zone from that variable at launch, which fits the session-scoped
driver.

## Verification status

Verified by execution: the UTC row and the Paris and Los Angeles rows quoted above, produced by
`clone-work/scratch/tz_matrix.js` under node v24.21.0.

Verified by reading: the absence of timezone configuration, the fixture scope, the test
assertions and their line numbers, and the mocha-in-node runner for bokehjs unit tests.

Not verified: the Selenium suite, a browser and the project build are unavailable in this
environment, so none of the three tests was run at either commit, and the proposed test code was
not executed. That Chrome picks up `TZ` at launch is from general knowledge of Chrome on Linux,
not from anything checked here.
