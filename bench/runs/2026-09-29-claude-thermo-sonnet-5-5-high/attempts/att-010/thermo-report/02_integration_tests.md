# Detail 02 — `tests/integration/widgets/test_datepicker.py`

Scope: new 98-line Selenium integration file, added across commits 4 (author's shell) and 5 (maintainer's rewrite).

## Finding D — the tests do not exercise the bug and cannot catch Finding A (test-coverage)

The regression in issue #9129 is about the *displayed* value under a non-UTC timezone. Coverage of that path:

- `test_basic` only checks the label text. It never reads the input's displayed value on first render, which is the path Finding A breaks in UTC− zones.
- `test_js_on_change_executes` does assert the displayed value after a click (`'Mon Sep 16 2019'`), which is the post-click path fixed in UTC+ zones. That is the path a UTC+ user hits. It passes vacuously in a UTC CI runner, since the old code was already correct at UTC.
- `test_server_on_change_round_trip` checks `old` and `new` via `datetime.utcfromtimestamp(...)`. Nothing checks the rendered text.
- Nothing sets `TZ` for the browser (for example via a Chrome `--timezone`/`TZ` env in the driver fixture, or `Emulation.setTimezoneOverride`). Maintainer suggested this idea in the PR conversation; it was not done. The suite therefore only ever runs in the CI machine's zone, so the fix is unverified in CI for exactly the zones it targets.

Remedy: parametrise the two display-checking tests over at least one UTC+ zone and one UTC− zone, and add an initial-render assertion (`el.get_attribute('value') == 'Fri Sep 20 2019'` after `bokeh_model_page`) using a Python-provided `datetime(2019, 9, 20)`. With those in place the Finding A regression would fail on the PST leg.

## Finding E — duplicated fixture construction and stale boilerplate (legibility, minor)

The identical `DatePicker(title='Select date', value=datetime(2019, 9, 20), min_date=datetime(2019, 9, 1), max_date=datetime.utcnow(), css_classes=["foo"])` line appears three times. It should be a small factory or fixture. `max_date=datetime.utcnow()` makes the tests time-dependent: clicking day 16 works only while "now" is on or after Sep 16 of a month that Pikaday is showing, which is fine for 2019 dates but is a needless moving part. Use a fixed `max_date`. The header still says "2012 - 2017" although the file is new in 2019, which follows the copied template. The `test_server_on_change_round_trip` case builds a `Plot`, `ColumnDataSource`, `Circle` and `CustomAction` only to smuggle values back to the test. That mirrors `test_radio_button_group.py`, so it is house style, but it is heavy for what it asserts. Using `bokeh_server_page` with only a `dp` and reading the model via `page.results` from a `CustomJS` would be shorter.

## Verification status

By inspection only. The Selenium suite and a browser were unavailable in this review.
