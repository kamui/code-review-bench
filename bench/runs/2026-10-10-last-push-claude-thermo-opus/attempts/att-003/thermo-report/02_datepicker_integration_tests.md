# 02 — DatePicker integration tests (`tests/integration/widgets/test_datepicker.py`)

Covers findings F3 and F4 of `summary.md`.

The file is new, 98 lines, and follows the layout of the sibling widget tests (`tests/integration/widgets/test_radio_button_group.py` was the stated model). The licence header, the `pytest_plugins` block and the plot/`CustomAction`/`RECORD` scaffolding in the server test are the directory's existing convention and are not raised here.

None of this file could be executed: the Selenium suite, a browser and the project build were unavailable. Everything below comes from reading the test and from the node measurements recorded in `01_datepicker_view.md`.

## F3 — what the three tests actually assert

`test_basic` (`:42-50`) checks the label text and the absence of console errors. It never looks at the date in the input.

`test_js_on_change_executes` (`:52-70`) opens the picker, clicks `button[data-pika-day="16"]`, then asserts that the recorded `cb_obj.value` is `'Mon Sep 16 2019'` (`:65`) and that the input shows `'Mon Sep 16 2019'` (`:68`). This is the only assertion in the file about displayed text, and it is taken after the click.

`test_server_on_change_round_trip` (`:72-98`) clicks the same day and checks the old and new values as seen by the Python callback (`:95-98`). Those are model values on the server; they do not pass through `_unlocal_date` and were correct before this PR, as the issue reporter says ("the underlying value responds well").

Set against the measurement table in `01_datepicker_view.md`:

| Runner timezone | Path the tests exercise (string after a pick) | Path the tests do not exercise (Python value on first render) |
| --- | --- | --- |
| UTC | merge-base correct, head correct | merge-base correct, head correct |
| East of UTC | merge-base **wrong**, head correct | merge-base correct, head correct |
| West of UTC | merge-base correct, head correct | merge-base correct, head **wrong** |

Under `TZ=UTC` `getTimezoneOffset()` is zero, the two added lines are a no-op, and the node run shows identical output for every input. A UTC runner therefore passes all three tests on the merge-base as well as on the head, and the tests are not a regression test for #9129 there. A runner west of UTC passes all three tests on the head while the widget's first render is a day early, because the only display assertion sits on the one path that is correct on both sides of UTC. The tests can fail only on the merge-base on a runner east of UTC.

The browser fixture (`bokeh/_testing/plugins/selenium.py:62-67`) starts headless Chrome with no timezone configuration, so the zone is whatever the host has.

### Worked proposal

Assert the first render, which is the assertion that catches the regression in F1:

```python
    def test_displays_initial_value(self, bokeh_model_page):
        page = bokeh_model_page(_date_picker())

        el = page.driver.find_element_by_css_selector('.foo input')
        assert el.get_attribute('value') == 'Fri Sep 20 2019'

        assert page.has_no_console_errors()
```

and keep the post-click assertion at `:68`. Then make the zone explicit instead of inherited. How to do that in this harness was not tried here; the two usual routes are starting the driver with `TZ` in its environment for a parametrised set of zones, or overriding the timezone through the browser's devtools protocol. One zone east and one west of UTC is the minimum that distinguishes the merge-base, the head and a correct implementation from one another.

The cheaper and more complete option follows from proposal A in `01_datepicker_view.md`. Once the decode takes the raw model value and has no dependence on the view, it can be unit-tested in bokehjs (`bokehjs/test/models/widgets/` already holds tests for other widgets) with the test process started under different `TZ` values. That covers the full matrix of wire shape × zone in milliseconds, and leaves the Selenium test as one end-to-end check that the decoded date reaches the input.

## F4 — the fixture

The constructor line appears three times, identical to the character:

```
:43  dp = DatePicker(title='Select date', value=datetime(2019, 9, 20), min_date=datetime(2019, 9, 1), max_date=datetime.utcnow(), css_classes=["foo"])
:53  dp = DatePicker(title='Select date', value=datetime(2019, 9, 20), min_date=datetime(2019, 9, 1), max_date=datetime.utcnow(), css_classes=["foo"])
:78  dp = DatePicker(title='Select date', value=datetime(2019, 9, 20), min_date=datetime(2019, 9, 1), max_date=datetime.utcnow(), css_classes=["foo"])
```

Three things are wrong with it beyond the repetition.

`max_date=datetime.utcnow()` makes the fixture a function of the wall clock. The tests click 16 September 2019, which will always be below "now", so the value adds nothing a constant would not, and it removes reproducibility.

The values are `datetime` objects, while the property is documented as `Accept Date (but not DateTime) values` (`bokeh/core/property/datetime.py:51-52`). `Date.transform` lets them through because `datetime` subclasses `date`, and they go onto the wire with their time of day intact (`bokeh/util/serialization.py:177-180`).

Under the head code that time of day matters. The node table in `01_datepicker_view.md` shows `datetime(2019, 9, 20, 23, 0)` decoding to `Sat Sep 21 2019` in `Europe/Paris` and `Asia/Kolkata`, and `datetime(2019, 9, 20, 3, 0)` decoding to `Thu Sep 19 2019` in `America/Los_Angeles` and `America/New_York`. With `utcnow()` as `max_date`, the upper bound the picker enforces therefore depends on both the runner's zone and the hour the test starts. It does not flip any current assertion, because 16 September is far from the bound, but it is exactly the kind of latent nondeterminism a test of a date widget should not carry.

### Worked proposal

```python
from datetime import date, datetime

VALUE = date(2019, 9, 20)
MIN_DATE = date(2019, 9, 1)
MAX_DATE = date(2019, 9, 30)

def _date_picker():
    return DatePicker(title='Select date', value=VALUE, min_date=MIN_DATE, max_date=MAX_DATE, css_classes=["foo"])
```

Each test then begins with `dp = _date_picker()`. This mirrors how `test_radio_button_group.py` hoists `LABELS` to module level. The server test's `d0`/`d1` checks at `:95-98` keep working unchanged, since a `date` serialises to the same UTC-midnight milliseconds the `datetime(2019, 9, 20)` did.

## Verification status

| Claim | Status |
| --- | --- |
| No test asserts the initially displayed date | Verified by reading the file |
| Head and merge-base decode identically under UTC | Verified by execution in node (`01_datepicker_view.md`) |
| Tests pass on the merge-base under UTC and on the head west of UTC | Inferred from the two rows above; Selenium suite not run |
| Constructor line is triplicated verbatim | Verified by reading the file |
| A time of day in `max_date` shifts the decoded bound by zone | Verified by execution in node on the decode function; not observed in a browser |
| The proposed test additions pass against a corrected view | Not run |
