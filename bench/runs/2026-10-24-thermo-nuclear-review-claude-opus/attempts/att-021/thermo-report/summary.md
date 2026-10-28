# Thermo-nuclear review — bokeh/bokeh#9232 (`ccb4bcb4c..36549bca3`)

Scope: `bokehjs/src/lib/models/widgets/date_picker.ts` (`_unlocal_date` rewrite) and the new `tests/integration/widgets/test_datepicker.py`. No security, devex or feature-gate concerns: the change is client-side date arithmetic plus a Selenium test file. Two actionable findings.

## Finding 1 — High: UTC− users now see Python-supplied `value`, `min_date` and `max_date` one day early

**File:** `bokehjs/src/lib/models/widgets/date_picker.ts`, lines 82–83 (called from lines 68, 70 and 71)

**Evidence:** `_unlocal_date` receives two kinds of input that encode a date differently.

- **Values from Python** are numbers set at UTC midnight. `DatePicker.value`, `min_date` and `max_date` are `Date` properties (`bokeh/models/widgets/inputs.py:253-263`). `Date.transform` turns strings and timestamps into `datetime.date` (`bokeh/core/property/datetime.py:58-69`). The JSON encoder then sends every date/datetime as `convert_datetime_type(obj)`, which is milliseconds since a naive UTC epoch (`bokeh/core/json_encoder.py:185-186`, `bokeh/util/serialization.py:178-184`). So `date(2019, 9, 20)` reaches the browser as `Date.UTC(2019, 8, 20)`. This applies to the initial document and to any later server-side assignment such as `dp.value = date.today()` in a callback.
- **Values chosen in the browser** are strings at local midnight. `_on_select` stores `date.toDateString()` (line 95), e.g. `"Mon Sep 16 2019"`. The `change` listener re-renders (line 54), and `new Date("Mon Sep 16 2019")` parses that string as local midnight.

The old code read the UTC calendar date. That was right for the numeric inputs in every timezone, but wrong for the string inputs in UTC+ zones (the bug in #9129). The new code always shifts by `-getTimezoneOffset()` before reading the UTC date. That fixes the string inputs everywhere, but in every UTC− zone it moves the numeric UTC-midnight inputs back into the previous day.

I ran the old and new function bodies verbatim under node with `TZ` set:

| TZ | input | old | new |
|---|---|---|---|
| America/Los_Angeles | `date(2019,9,20)` → `Date.UTC(2019,8,20)` | Fri Sep 20 | **Thu Sep 19** |
| America/New_York | same | Fri Sep 20 | **Thu Sep 19** |
| America/Sao_Paulo | same | Fri Sep 20 | **Thu Sep 19** |
| Pacific/Pago_Pago | same | Fri Sep 20 | **Thu Sep 19** |
| Europe/Paris | `"Mon Sep 16 2019"` | Sun Sep 15 (bug #9129) | Mon Sep 16 (fixed) |
| America/Los_Angeles | `"Mon Sep 16 2019"` | Mon Sep 16 | Mon Sep 16 |

**Failure:** Take a user anywhere in the Americas (or any other UTC− zone) and `DatePicker(value=date(2019, 9, 20), min_date=date(2019, 9, 1), max_date=date(2019, 9, 30))`. Then:

- Pikaday's `defaultDate` with `setDefaultDate: true` writes "Thu Sep 19 2019" into the input box on first render, although the model value is the 20th.
- `minDate` becomes Aug 31, so the user can pick a day the app meant to forbid.
- `maxDate` becomes Sep 29, so the user cannot pick Sep 30, a day the app allows.
- Whenever server code sets `dp.value`, the display again shows the previous day.

This is the same off-by-one-day symptom #9129 reported, now moved from UTC+ users to UTC− users. It goes in a release that worked correctly for them before. The PR's own manual check ("working great for me in PST") and the new tests only covered dates picked in the browser (string inputs), so this path was never exercised. A fix has to tell the two encodings apart. Numbers (UTC midnight) should be read with `getUTCFullYear/Month/Date`. Strings from `toDateString()` (local midnight) should be read with the local getters. Better still, serialize `value` in one unambiguous format.

## Finding 2 — Medium: the new integration tests cannot detect the bug they were added for, or the regression above

**File:** `tests/integration/widgets/test_datepicker.py`, lines 53–98

**Evidence:**
- The browser timezone is never set. The test file does not set it, and the CI config does not either (`.travis.yml` and `scripts/ci/*` contain no `TZ` setting, so Travis runs in UTC).
- When the offset is 0, `getTimezoneOffset()` is 0 and the new lines 82–83 do nothing. The old and new `_unlocal_date` then give the same results for every input (first rows of the node run above). So `test_js_on_change_executes` (asserting `'Mon Sep 16 2019'` in the input) and `test_server_on_change_round_trip` pass with or without the fix.
- No test checks what the input shows on first render for a Python-supplied `value`, and none checks the `min_date`/`max_date` bounds. Those are exactly the paths broken in Finding 1.
- `max_date=datetime.utcnow()` makes the fixtures depend on the wall clock, and it passes a `datetime` to a `Date` property. Harmless today, but the tests are not deterministic.

**Failure:** The suite passes against the pre-fix code and against the regressed code alike. It gives false confidence that #9129 is covered, and it will not catch Finding 1 or a reintroduction of #9129. To be meaningful, the tests need to run the browser under a non-UTC timezone, at least one UTC+ and one UTC− (e.g. via the driver's environment or CDP `Emulation.setTimezoneOverride`). They should also assert the input's initial displayed value and that the min/max days are enabled or disabled as expected.
