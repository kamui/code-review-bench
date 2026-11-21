# Thermo-nuclear review — bokeh/bokeh#9232

Range reviewed: `ccb4bcb4c2b841d89b0e88303a97bf4604a5795f..36549bca3a63d581f7b68d08054a7813c1e6a499` (`main...review-head`), 2 files: `bokehjs/src/lib/models/widgets/date_picker.ts` and the new `tests/integration/widgets/test_datepicker.py`.

The change fixes the reported symptom (after picking a day in a UTC+ browser, the field showed the day before) but does it by applying the local-timezone shift to every value that passes through `_unlocal_date`, including the UTC-midnight timestamps that Python sends. That moves the off-by-one from UTC+ browsers after a selection to UTC− browsers on initial render, and also shifts `min_date`/`max_date` there. Two findings: one High, one Low.

How this was checked: the code paths were read in the clone, and the old and new bodies of `_unlocal_date` were copied into a scratch Node script and run under eleven `TZ` settings. The bokehjs build, the Selenium suite and a browser were not available, so nothing was exercised in a real page. Pikaday's source is not vendored in the clone (`bokehjs/package.json:88` points at a git fork and `node_modules` is absent), so statements about what Pikaday does with `minDate`/`maxDate` rest on its documented behaviour, not on a local read.

## Findings

### 1. [High] `_unlocal_date` now shows Python-supplied dates one day early in every browser west of UTC, and shifts `min_date`/`max_date` with them

**File:** `bokehjs/src/lib/models/widgets/date_picker.ts`, lines 78–88 (the added lines are 82–83).

**Evidence.** `_unlocal_date` receives two different kinds of input, and the added shift is right for only one of them.

- Values that come from Python are numbers holding UTC midnight. `DatePicker.value`, `min_date` and `max_date` are `Date` properties (`bokeh/models/widgets/inputs.py`), `Date.transform` leaves `date`/`datetime` objects untouched (`bokeh/core/property/datetime.py:58–68`), and the JSON encoder turns them into milliseconds since the UTC epoch (`bokeh/util/serialization.py:177–184`, `DT_EPOCH = utcfromtimestamp(0)` at line 83). `date(2019, 9, 20)` therefore reaches the browser as `1568937600000`. On the JS side the properties are `p.Any` (lines 124–126), so `render()` calls `this._unlocal_date(new Date(1568937600000))` (lines 68, 70, 71).
- Values written by the widget itself are strings holding a local date: `_on_select` stores `date.toDateString()` (line 95), and `new Date("Mon Sep 16 2019")` parses as local midnight. The JS default (`new Date().toDateString()`, line 124) is the same kind.

The old body read the UTC calendar date (`toISOString().substr(0, 10)`), which is correct for the numeric kind in every timezone and wrong for the string kind in UTC+ zones — the bug in #9129. The new lines 82–83 subtract `getTimezoneOffset()` first, which is correct for the string kind in every timezone, but for a UTC-midnight number it produces the previous day whenever the offset is positive, i.e. in any zone behind UTC.

Scratch run of the old and new function bodies (Node, `TZ` set per row; input → resulting local date):

| Input | TZ | Old | New |
| --- | --- | --- | --- |
| `1568937600000` (Python `date(2019, 9, 20)`) | America/Los_Angeles | Fri Sep 20 2019 | **Thu Sep 19 2019** |
| same | America/New_York | Fri Sep 20 2019 | **Thu Sep 19 2019** |
| same | America/Sao_Paulo | Fri Sep 20 2019 | **Thu Sep 19 2019** |
| same | Pacific/Pago_Pago | Fri Sep 20 2019 | **Thu Sep 19 2019** |
| `1547510400000` (Python `date(2019, 1, 15)`) | Atlantic/Azores (UTC−1 in winter) | Tue Jan 15 2019 | **Mon Jan 14 2019** |
| `1567296000000` (Python `min_date=date(2019, 9, 1)`) | America/Los_Angeles | Sun Sep 01 2019 | **Sat Aug 31 2019** |
| `1568937600000` | UTC, Europe/London, Europe/Paris, Asia/Kolkata, Asia/Tokyo, Pacific/Kiritimati | Fri Sep 20 2019 | Fri Sep 20 2019 |
| `"Mon Sep 16 2019"` (after a selection) | Europe/London, Europe/Paris, Asia/Kolkata, Asia/Tokyo, Pacific/Kiritimati | Sun Sep 15 2019 | Mon Sep 16 2019 |
| `"Mon Sep 16 2019"` | all UTC− zones above | Mon Sep 16 2019 | Mon Sep 16 2019 |

**Failure.** For any user whose browser is behind UTC (all of the Americas, Hawaii, and others such as the Azores and Cape Verde), on every DatePicker whose value comes from Python — which is every DatePicker created from Python, since the property always serialises to a number:

- The field and the highlighted calendar day are one day before the configured `value` on first render. The issue's own example, `DatePicker(value=datetime.date.today())`, shows yesterday. The Python model still holds the correct date, so what the user sees and what a server callback or a "submit" button reads disagree until the user picks a day.
- The same happens each time Python assigns `dp.value` in a server app: the pushed number triggers `render()` (line 54) and the field shows the previous day.
- `minDate` and `maxDate` are computed from numbers on every render, including renders after a selection, so they stay one day early for the life of the widget. With `min_date=date(2019, 9, 1)` the picker's lower bound becomes Aug 31, so a day before the application's minimum can be picked and is sent to Python (which does not re-check the range); with `max_date=date(2019, 9, 30)` the upper bound becomes Sep 29 and the last allowed day cannot be picked.

None of this happened before the change in those zones: the old code was correct there for both kinds of input. The off-by-one has been moved from UTC+ users after a selection to UTC− users on initial display, and the range limits are now wrong as well. The new comment on lines 79–81 ("agnostic to their local systems's timezone") describes the opposite of what the code does for numeric input.

A smaller effect of the same lines in UTC+ zones: a numeric value that carries a time of day (the `Date` property accepts `datetime`, and this PR's tests pass `max_date=datetime.utcnow()`) used to map to its UTC calendar date and now maps to the next day once UTC time plus the offset passes midnight. Scratch run: `datetime(2019, 9, 20, 23, 30)` gives Sep 20 under the old code everywhere, and Sep 21 under the new code in London (BST), Paris, Kolkata, Tokyo and Kiritimati.

**Direction for a fix.** The shift should apply only to the locally-parsed string form (or `_on_select` and the JS default should store a form that the UTC read handles); UTC-midnight numbers need the old UTC read.

### 2. [Low] The new integration tests cannot detect either the original bug or the regression above

**File:** `tests/integration/widgets/test_datepicker.py`, lines 52–71 (`test_js_on_change_executes`); also lines 42–50 and 73–98.

**Evidence.** The only assertion on the displayed date is line 69, made after clicking day 16. Per the table in finding 1, the string `"Mon Sep 16 2019"` converts to Sep 16 under both the old and the new `_unlocal_date` when the browser is at UTC or behind it; only a UTC+ browser separates them. Nothing in the test or in `bokeh/_testing/plugins/bokeh.py` sets the browser's timezone, so on a UTC CI host this test passes with the fix reverted. No test asserts the field's text before a selection (`value=datetime(2019, 9, 20)` should display `Fri Sep 20 2019`), and none asserts which days `min_date`/`max_date` enable. `test_server_on_change_round_trip` checks only the values Python receives, which come from `toDateString()` and do not go through `_unlocal_date`.

**Failure.** The suite added for this fix is green in every timezone whether the fix is present or not, and stays green with the UTC− regression in finding 1, so neither the bug being fixed nor the one introduced is guarded. An assertion on the initial field text, run with the browser in one UTC+ and one UTC− zone, would cover both.

## Checked and found sound

- `_unlocal_date` now mutates its argument via `setTime`; all three call sites (lines 68, 70, 71) pass a fresh `new Date(...)`, so nothing else observes the mutation.
- The fix does what it claims for the reported case: a string value parsed as local midnight converts to the same calendar day in all eleven zones tried, including UTC+14.
- `.idea/vcs.xml` was added in the first commit and removed in the third; it is not in the net diff.
- The test file's imports are all used, its selectors and helpers (`RECORD`, `page.results`, `click_custom_action`, `has_no_console_errors`) exist in `bokeh/_testing`, and the server round-trip values (`datetime` for the old value, `date` parsed by `Date.transform` for the new one) serialise to the milliseconds the assertions expect.
- No security, developer-workflow or feature-gating impact: no new dependencies, environment variables, ports or flags.
