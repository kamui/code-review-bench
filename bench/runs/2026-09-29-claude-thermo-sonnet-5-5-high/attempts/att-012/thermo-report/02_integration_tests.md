# Detail 02 — `tests/integration/widgets/test_datepicker.py`

Scope: new file, 98 lines. Not executed: Selenium and a browser were unavailable.

## Finding C — the tests do not pin the bug they were added for (verified by reading)

The fix is about the day shown in the input for users in UTC+ zones, and, as shown in detail 01, is timezone-dependent in both directions. The tests never vary the timezone, so on a UTC CI machine they pass with old and new code alike.

- `test_basic` (lines 42-50) asserts only the label text.
- `test_js_on_change_executes` (lines 52-70) does assert the displayed input value after a click, but it only covers the click path (shape 2 in detail 01). It never checks the initial rendering of `value=datetime(2019, 9, 20)`. That check would have failed in UTC- zones with this patch.
- `test_server_on_change_round_trip` (lines 72-98) checks the model values, not the display.

There is no assertion that the input's initial value equals "Fri Sep 20 2019", and no run under a non-UTC `TZ` (for example browser launch env or a parametrized timezone). That leaves Finding A undetected.

## Finding D — test data and structure (minor)

The same `DatePicker(title='Select date', value=datetime(2019, 9, 20), min_date=datetime(2019, 9, 1), max_date=datetime.utcnow(), css_classes=["foo"])` construction is copy-pasted at lines 43, 53 and 78; a small factory or fixture would remove it. `max_date=datetime.utcnow()` makes the tests depend on the wall clock. It also depends on the current date being after 16 Sep 2019, so a pinned constant is more robust. `test_basic` duplicates what the other two tests already exercise (it renders and checks no console errors).

## Remedy

Add a test that renders the picker with a fixed `value` and asserts the input text before any click. Run it in at least one UTC+ and one UTC- zone. Replace `utcnow()` with a fixed date. Build the widget once via a helper.
