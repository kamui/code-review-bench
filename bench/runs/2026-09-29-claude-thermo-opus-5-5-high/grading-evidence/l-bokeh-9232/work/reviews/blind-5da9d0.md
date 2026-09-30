# Review blind-5da9d0

### Item 1
Location: bokehjs/src/lib/models/widgets/date_picker.ts:82-83
Claim: In `bokehjs/src/lib/models/widgets/date_picker.ts` lines 82-83, the new `getTimezoneOffset()` shift makes `_unlocal_date` read every value's calendar day in the browser's local zone. But `value`, `min_date` and `max_date` set from Python arrive as milliseconds at **UTC midnight** (`convert_datetime_type` in `bokeh/util/serialization.py:182-183`). West of Greenwich those resolve to the previous local day. I ran the old and new helper bodies under node with `TZ=America/Los_Angeles` and `TZ=Pacific/Pago_Pago`. A Python `date(2019, 9, 20)` showed Sep 20 before the PR and shows **Sep 19** after it. `min_date`/`max_date` move back one day the same way, because `render()` sends all three through this helper (lines 68-71). Pikaday sets the default date without firing `onSelect`, so the model keeps the right value while the UI shows the wrong one. That is exactly the #9129 symptom, just in the other hemisphere. The manual PST check in the PR thread tested the select path, which the PR does fix, not the initial display. Remedy: adopt Finding 2. Full evidence and command output are in `01_date_picker_ts.md` § Finding 1.1. **Status: confirmed by execution.**
Consequence: —
Fix: —

### Item 2
Location: bokehjs/src/lib/models/widgets/date_picker.ts:90-96
Claim: The real defect is that one model property carries two encodings in two time zones: Python sends UTC-midnight milliseconds, and `_on_select` (lines 90-96) and the default (line 124) write `toDateString()`, which parses as local midnight. The loose `p.Property<string>` over `p.Any` typing (lines 104-106, 123-126) hides this. The PR adds offset arithmetic to the reader and keeps both encodings. The simpler fix removes the second encoding. `_on_select` should write a `YYYY-MM-DD` string built from Pikaday's local fields. ECMAScript parses that string as UTC midnight, so every value becomes a UTC-midnight day, and a single pure `to_picker_date(value: string | number)` that reads UTC fields is correct everywhere. That removes the offset math, the in-place mutation, the ISO-slice-split round trip, and the confusing "toISOString returns the wrong day" warning. Python's `Date.transform` already parses such strings. I verified this proposal (A) under five zones from Kiritimati (UTC+14) to Pago Pago (UTC−11), and it gives the right day for both sources. The only visible change is that `cb_obj.value` in `CustomJS` becomes `"2019-09-16"` instead of `"Mon Sep 16 2019"`. If that format counts as public, proposal B keeps it: one `typeof value == "number"` dispatch (UTC fields for numbers, local fields for strings). It passed the same check in every zone. Worked code for both is in `01_date_picker_ts.md` § Finding 1.2. **Status: proposal verified by execution.**
Consequence: —
Fix: —

### Item 3
Location: bokehjs/src/lib/models/widgets/date_picker.ts:78-88
Claim: After the change, the helper at lines 78-88 shifts the instant, serializes it to ISO, slices, splits, calls `Number()`, and rebuilds a `Date`. Algebraically, and confirmed by execution in every zone tested, that is just `new Date(d.getFullYear(), d.getMonth(), d.getDate())`. Along the way it calls `date.setTime(...)` on its **argument**: the scratch run reports `arg_mutated=true` in every non-UTC zone. That is harmless today only because each caller passes a fresh `new Date(...)`. It is a trap for the next caller of a function that reads like a pure conversion. The method name `_unlocal_date` now describes the opposite of what it does. The new comment says the result is "agnostic to their local systems's timezone", which Finding 1 shows is false. Remedy: replace the method with the pure, typed `to_picker_date` from Finding 2. At minimum, write it as the one-line field read, with no mutation and an accurate name and comment. Details are in `01_date_picker_ts.md` § Finding 1.3. **Status: confirmed by execution.**
Consequence: —
Fix: —

### Item 4
Location: tests/integration/widgets/test_datepicker.py:42-98
Claim: `tests/integration/widgets/test_datepicker.py` (lines 42-98) only checks the displayed value after clicking day 16. #9129 needs a UTC+ browser zone. In UTC, which is most likely the CI zone, the old and new helpers agree on every input (`TZ=UTC` run), so these tests would pass with the fix reverted. `test_basic` sets `value=datetime(2019, 9, 20)` but never asserts on the initially displayed input value, and that is exactly where Finding 1's regression appears. The meaningful coverage is a node-side unit test of the pure conversion function in `bokehjs/test/models/widgets/`, written with zone-independent assertions and run under both a UTC+ and a UTC− `TZ`. The Selenium test should also assert the initial `.bk-input` value. Details are in `02_integration_tests.md` § Finding 2.1. **Status: confirmed by reasoning plus helper execution; the Selenium suite was not run (unavailable).**
Consequence: —
Fix: —

### Item 5
Location: tests/integration/widgets/test_datepicker.py:43-79
Claim: The same `DatePicker(title=..., value=datetime(2019, 9, 20), min_date=datetime(2019, 9, 1), max_date=datetime.utcnow(), css_classes=["foo"])` expression is repeated at lines 43, 53 and 79 of the test file. `max_date=datetime.utcnow()` ties the fixture to the wall clock and sends a non-midnight `datetime` into a `Date` property. That is the least-understood input shape for the conversion code. Use a module-level factory with a fixed `date(...)` bound, or drop `max_date`. Details are in `02_integration_tests.md` § Finding 2.2. **Status: confirmed by reading.** (Low severity.)
Consequence: —
Fix: —
