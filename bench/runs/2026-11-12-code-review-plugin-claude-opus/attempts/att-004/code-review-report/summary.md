# Code review — bokeh/bokeh#9232

Range: `ccb4bcb4c2b841d89b0e88303a97bf4604a5795f..36549bca3a63d581f7b68d08054a7813c1e6a499` (`main...review-head`)

Checked for bugs and CLAUDE.md compliance. No CLAUDE.md applies to the changed files. Step 6 kept 1 issue.

## Issue 1: Timezone shift in `_unlocal_date` moves Python-set dates back one day for UTC− users

- **Reason flagged:** bug
- **File / lines:** `bokehjs/src/lib/models/widgets/date_picker.ts` lines 82–83. The same function is called from `render()` at lines 68, 70 and 71.
- **Description:** The new code runs `date.setTime(date.getTime() - date.getTimezoneOffset() * 60000)` before reading the day with `toISOString().substr(0, 10)`. That fixes the date string `_on_select` writes back via `toDateString()` (for example `"Mon Sep 16 2019"`), which `new Date()` parses as local midnight. But `_unlocal_date` also gets the initial `value`, `min_date` and `max_date` sent from Python. Those are `Date()` properties, which `bokeh/util/serialization.py` `convert_datetime_type` serializes as millisecond timestamps at UTC midnight.
- **Why it fails:** In UTC− timezones (the Americas), `getTimezoneOffset()` is positive. Subtracting it moves UTC midnight back into the evening of the previous UTC day, so `toISOString()` returns the previous date. The old code read these timestamps correctly in every timezone. Node, run with the old and new function bodies, shows this for `Date.UTC(2019, 8, 16)`:
  - **America/New_York and America/Los_Angeles:** the old code gives Mon Sep 16; the new code gives Sun Sep 15.
  - **Europe/Paris and Asia/Tokyo:** both versions give Sep 16 for timestamps. Only the string input was wrong there, which is the #9129 case.

  The effect for UTC− users is that `DatePicker(value=date(2019, 9, 16), ...)` shows Sep 15 as the default date. `minDate` and `maxDate` are also one day early: the real maximum day can't be selected and the day before the minimum can. The bug moves from UTC+ users to UTC− users instead of being fixed for both input formats.
