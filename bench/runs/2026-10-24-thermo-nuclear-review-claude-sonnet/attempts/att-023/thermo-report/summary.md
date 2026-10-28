# Thermo Nuclear Review — bokeh/bokeh#9232

## Finding 1 (High): `_unlocal_date` now shifts Python-supplied dates one day early for users in UTC− timezones (Americas)

**File:** `bokehjs/src/lib/models/widgets/date_picker.ts`, lines 78–88 (the new offset adjustment is at lines 82–83). It is applied to `value`, `min_date` and `max_date` at lines 68–71.

**Evidence traced:**
- `DatePicker.value`, `min_date` and `max_date` are `Date` properties in `bokeh/models/widgets/inputs.py`. Python dates and datetimes are serialized by `convert_datetime_type` in `bokeh/util/serialization.py`. That function returns epoch milliseconds for UTC midnight. For a `date` it computes `datetime(...) - DT_EPOCH`.
- In BokehJS the property is `p.Any`, so the value stays a number. `new Date(number)` is therefore UTC midnight of the intended day.
- The old code read `toISOString().substr(0,10)`, which is the UTC calendar day. That is correct for this input in every timezone.
- The new code first runs `date.setTime(date.getTime() - date.getTimezoneOffset()*60000)`. This only works if the input is a local-midnight `Date`, which is what the `toDateString()` string written by `_on_select` produces (e.g. "Mon Sep 16 2019"). It does not work for UTC-midnight input. For UTC− zones `getTimezoneOffset()` is positive, so the subtraction moves the instant back across the UTC day boundary and the extracted day is one earlier.
- I ran the old and new functions in node for a Python `date(2019,9,20)`, which is `Date.UTC(2019,8,20)`:
  - `TZ=America/Los_Angeles`: old displays Fri Sep 20 (correct), new displays **Thu Sep 19** (wrong).
  - `UTC`, `Europe/Paris` and `Pacific/Auckland`: new displays Fri Sep 20 (correct).
  - For the local-midnight string "Mon Sep 16 2019", new is correct in all zones. Old was wrong in Paris and Auckland, which is the bug this PR fixes.

**Failure:** A user in the Americas, or anywhere UTC−, opens an app with `DatePicker(value=date(2019,9,20))`. The initial display is one day earlier than the model value, and this also happens on every render triggered by a server-side change to `value`. `min_date` and `max_date` shift by a day the same way, so the day at the boundary of the allowed range is wrongly disabled or enabled.

This is the same symptom as #9129 with the direction reversed. It regresses users who were previously correct, and the reviewer's PST check only exercised the selected-string path. The fix needs to treat the two input forms consistently. For example, parse the input so both forms yield the intended Y/M/D. A numeric or ISO input should use UTC fields. A `toDateString()` string should use local fields.

## Finding 2 (Medium): The added integration tests don't cover the changed behavior

**File:** `tests/integration/widgets/test_datepicker.py`, lines 38–98.

**Evidence:**
- No test asserts the initially displayed text of the input for a Python-supplied `value`. That is exactly the path `_unlocal_date` changes and the one that regresses in Finding 1. The only assertion on displayed text, in `test_js_on_change_executes`, comes after a click, when the value is a local-midnight string.
- No test varies the browser timezone. The suite would pass in both a UTC+ environment (bug present before the fix) and a UTC− environment (bug introduced by the fix) without detecting either.
- `max_date=datetime.utcnow()` makes the test depend on the current date. Under the new code in UTC− zones it is shifted a day earlier.

**Failure:** The suite cannot catch the regression in Finding 1, and it gives false confidence that the #9129 fix is verified. These tests also need Selenium, so they could not be run in this review. This finding is based on reading them.
