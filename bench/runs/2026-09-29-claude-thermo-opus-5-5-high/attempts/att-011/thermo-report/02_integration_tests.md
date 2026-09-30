# 02 — `tests/integration/widgets/test_datepicker.py` (new, 98 lines)

## Finding 2.1 — The new tests cannot fail on the bug they accompany, and they skip the path the fix regresses (lines 42-98)

**Status: CONFIRMED by reasoning plus execution of the helper; the Selenium suite itself was not run (unavailable).**

The three tests check the label text, the value after clicking day 16 (the JS callback
value and the input's `value` attribute), and the old/new values in a server round trip.
Two problems follow.

1. **They pass on the pre-PR code.** Issue #9129 only appears when the browser's local
   zone is ahead of UTC. The Selenium job runs in whatever zone the CI machine uses, most
   likely UTC. In UTC the old and new `_unlocal_date` return identical results for every
   input tried (`TZ=UTC node unlocal.js`: `old == new` on all five rows, see
   `01_date_picker_ts.md`). So `test_js_on_change_executes`, the only test that checks
   the displayed value, would pass with the fix reverted. That makes it a smoke test for
   the widget, not a regression test for #9129. The PR conversation raised the idea of
   running these tests under a different zone, but the PR does not do it.
2. **They never assert on the initially displayed date.** `test_basic` sets
   `value=datetime(2019, 9, 20)` and checks only the label. It never reads
   `.bk-input`'s `value`. That initial display is exactly where the PR regresses in
   UTC− zones (Finding 1.1). The regression is invisible in UTC CI and would stay
   invisible even under a UTC+ `TZ`.

Remedy. Put the date logic in a pure function (`to_picker_date` / `to_iso_date` in
`01_date_picker_ts.md`, Finding 1.2) and unit-test it in `bokehjs/test/models/widgets/`.
That directory already has mocha/chai tests running under node + jsdom (for example
`slider.ts`). Write assertions that hold in every zone, such as
`to_picker_date(Date.UTC(2019, 8, 20)).getDate() == 20` and a round trip through
`_on_select`'s output. Then run that suite at least once with `TZ=Europe/Paris` and once
with `TZ=America/Los_Angeles`, since node honors `TZ`. That covers both hemispheres
cheaply and deterministically, without a browser. The Selenium test should also assert
the initial `.bk-input` value (`Fri Sep 20 2019`, or `2019-09-20` under proposal A), so
the first-render path has browser coverage.

## Finding 2.2 — Copy-pasted, clock-dependent fixture construction (lines 43, 53, 79)

**Status: CONFIRMED by reading.**

The same long `DatePicker(title='Select date', value=datetime(2019, 9, 20),
min_date=datetime(2019, 9, 1), max_date=datetime.utcnow(), css_classes=["foo"])`
expression appears three times. `max_date=datetime.utcnow()` makes the fixture depend on
the wall clock and passes a `datetime` with a time of day into a `Date` property. The
picker then receives a non-midnight instant, which is the least-understood input shape
for the conversion code (see the caveat in Finding 1.2). None of the tests need an open
upper bound tied to "now". A fixed `max_date=date(2019, 10, 15)`, or no `max_date`,
together with a module-level factory (for example `def _make_picker(**kw)`) would remove
the duplication and make the tests deterministic. The fixtures should also use
`datetime.date` rather than `datetime.datetime`, because the property is a `Date`.

## Minor, non-blocking

- The header says `Copyright (c) 2012 - 2017`, and the file uses the Python 2 `__future__`
  boilerplate. That matches the neighbouring files at this base, so it is fine.
- `test_server_on_change_round_trip` builds a `Plot` + `CustomAction` just to read back
  `source.data`. This follows the existing pattern in `test_radio_button_group.py`, so it
  is not flagged.
