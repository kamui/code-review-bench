# Thermo-nuclear review — bokeh/bokeh#9232

Range reviewed: `ccb4bcb4c..36549bca3` (`main...review-head`), 2 files: `bokehjs/src/lib/models/widgets/date_picker.ts` (+6/−2) and `tests/integration/widgets/test_datepicker.py` (new, +98).

The change fixes the reported symptom (after picking a date, users east of UTC saw the previous day), but it does so by shifting every date that passes through `_unlocal_date`, including the ones that were already correct. That moves the off-by-one-day error onto users west of UTC, for the most common way the widget is used. The new tests cannot see either the fix or the regression on a UTC runner.

Verification: the old and new bodies of `_unlocal_date` were copied into a scratch script and run under node with `TZ` set to UTC, America/Los_Angeles, America/New_York, America/Sao_Paulo, Europe/Paris, Europe/London, Asia/Tokyo and Pacific/Kiritimati. The Python serialization path was traced by reading the code. The bokehjs build, the Selenium suite and a browser were unavailable, so nothing was run end to end.

## Findings

### 1. [High] `_unlocal_date` now shows Python-supplied dates one day early for every user west of UTC

**File:** `bokehjs/src/lib/models/widgets/date_picker.ts`, lines 78–88 (the added lines are 82–83; callers at lines 68, 70 and 71).

**Evidence.** `_unlocal_date` receives two kinds of `Date`, and the new offset is only right for one of them:

- *Local-midnight instants.* After a selection, `_on_select` stores `date.toDateString()` (line 95), and `new Date("Mon Sep 16 2019")` parses as local midnight. East of UTC that instant falls on the previous UTC day, so the old `toISOString()` slice returned the previous day. This is issue #9129, and subtracting `getTimezoneOffset()` fixes it.
- *UTC-midnight instants.* A value set from Python is a `datetime.date`, which `convert_datetime_type` (`bokeh/util/serialization.py:183–184`) serializes as milliseconds since the epoch at UTC midnight. `new Date(ms)` is then exactly `YYYY-MM-DDT00:00Z`, and the old code read the correct day from it in every timezone. An ISO string such as `"2019-09-20"` behaves the same way, because JavaScript parses date-only ISO strings as UTC.

The new lines 82–83 apply the local offset to both kinds. For a UTC-midnight instant in a timezone west of UTC, `getTimezoneOffset()` is positive, so the instant moves back into the previous UTC day and the function returns the day before.

Node results for the old and new function bodies:

| Input | Timezone | Old | New |
| --- | --- | --- | --- |
| `date(2019, 9, 20)` → `1568937600000` | America/Los_Angeles, America/New_York, America/Sao_Paulo | Fri Sep 20 2019 | **Thu Sep 19 2019** |
| ISO string `"2019-09-20"` | same three | Fri Sep 20 2019 | **Thu Sep 19 2019** |
| `date(2019, 1, 1)` | same three | Tue Jan 01 2019 | **Mon Dec 31 2018** |
| `"Mon Sep 16 2019"` (after select) | Europe/Paris, Europe/London, Asia/Tokyo | Sun Sep 15 2019 | Mon Sep 16 2019 (the intended fix) |
| `"Mon Sep 16 2019"` (after select) | the three western zones | Mon Sep 16 2019 | Mon Sep 16 2019 |
| `datetime(2019, 9, 20, 20, 30)` | Asia/Tokyo, Pacific/Kiritimati | Fri Sep 20 2019 | **Sat Sep 21 2019** |

Before this change, users west of UTC had no wrong-day behaviour on any of these paths.

**Failure.** For a browser in any UTC− timezone (all of the Americas):

- `DatePicker(value=date(2019, 9, 20))` initially displays and highlights Sep 19, while the model value is still Sep 20. The issue's own reproduction, `DatePicker(value=datetime.date.today())`, opens on yesterday. `examples/models/file/widgets.py:66` and `examples/integration/widgets/layout_multiple_widgets.py:52–54` are affected the same way.
- Every later update of `value` from a server callback re-renders through line 68 and shows the day before the one Python set.
- `min_date` and `max_date` (lines 70–71) shift one day early as well. The day before `min_date` becomes selectable and is sent to Python as a valid value outside the declared range, and the `max_date` day itself can no longer be picked.

A smaller second effect has the same cause: a `datetime` with a time-of-day component (accepted by the `Date` property because `datetime` subclasses `date`) used to display its UTC calendar day everywhere and now displays the next day in timezones far enough east of UTC, as in the last table row.

The offset should be applied only to instants that were parsed as local time (the `toDateString()` round trip), not to UTC-based timestamps and ISO strings. Alternatively the selected date could be stored in a UTC-based form so that a single conversion is right for every input.

### 2. [Low] The new integration tests cannot detect the fix or the regression above

**File:** `tests/integration/widgets/test_datepicker.py`, lines 52–70 (`test_js_on_change_executes`) and 72–98 (`test_server_on_change_round_trip`).

**Evidence.** Nothing in the test file, `.travis.yml` or `scripts/ci` sets the browser's timezone, so the tests run in whatever zone the runner uses. At `TZ=UTC`, `getTimezoneOffset()` is 0 and the node run shows the old and new `_unlocal_date` returning identical results for every input. The only assertion on displayed text is the `.bk-input` check at lines 68–69, which is made after the click. No test asserts the initially displayed value. `test_server_on_change_round_trip` asserts only values that travel through Python's UTC-based serialization, so it is independent of `_unlocal_date` altogether.

**Failure.** On a UTC runner the suite passes with the fix reverted, so it does not guard against #9129 returning. On a runner west of UTC it would still pass while the picker opens on the wrong day, so it does not catch finding 1 either. An assertion on the initial `.bk-input` value for a `date(...)` input, run under at least one UTC+ and one UTC− browser timezone, would cover both.

## Checked and found sound

- For the post-select path, the new arithmetic is correct in every tested zone, including zones where local midnight does not exist on a DST change day (the offset is read from the same instant it is applied to).
- `date.setTime(...)` mutates its argument, but all three callers pass a freshly constructed `Date`.
- The new test file's fixtures, selectors and `RECORD`/`results` usage match the sibling widget tests, and the server round-trip assertions hold in any server timezone because both values are serialized as UTC-midnight milliseconds.
- No security, developer-workflow or feature-gating impact: the change adds no dependencies, configuration, environment variables or scripts.
