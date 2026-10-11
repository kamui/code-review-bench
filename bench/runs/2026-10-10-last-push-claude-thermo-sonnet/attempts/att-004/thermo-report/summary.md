# Thermo-nuclear code quality review — bokeh/bokeh#9232

**Range:** `ccb4bcb4c..36549bca3` (`git diff main...review-head`), 2 files, +104/−2.
**Detail file:** `01_date_picker_widget.md`.

## Verdict

Do not approve as is. The change is small and has no file-size issue (`date_picker.ts` is 129 lines). But it fixes one input encoding by breaking the other, and it preserves a convoluted shift, stringify and re-parse mechanism when a much simpler normalization is available. The new integration tests do not exercise the broken path, and they cannot detect a timezone bug in a UTC CI environment.

## Findings

**1. The fix trades a UTC+ bug for a UTC− bug (`bokehjs/src/lib/models/widgets/date_picker.ts:78-88`).** `_unlocal_date` receives dates in two encodings. Python-supplied values are serialized as epoch milliseconds at UTC midnight (`bokeh/util/serialization.py:183-184`). Values picked in the widget are stored by `_on_select` as `toDateString()` text, which parses as local midnight. The old code was correct for the first and wrong for the second in UTC+ zones, which is #9129. The new code subtracts `getTimezoneOffset()` before reading the UTC fields, which is correct for the second encoding and wrong for the first in UTC− zones. I ran both versions under node with `TZ` set. For `Date.UTC(2019,8,20)`, which is what Python sends, the PR code yields `Thu Sep 19 2019` under `America/Los_Angeles`, where the old code yielded `Fri Sep 20`. `value`, `min_date` and `max_date` all go through this function, so a Python-set date, and the selectable range boundaries, can shift by one day for every user in the Americas. The browser render was not run, so the end-to-end display is inferred from the verified function behaviour. Full table and evidence are in detail file 01, Finding 1.

**2. Missed code-judo move: the whole function should disappear (`date_picker.ts:78-88`).** The root problem is that the code guesses the encoding after wrapping everything in `new Date(x)`. Normalize at the boundary instead: if the value is a number (UTC-midnight epoch milliseconds), build `new Date(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate())`. Otherwise `new Date(text)` already gives local midnight for `toDateString()` text. I ran this variant under UTC, Paris, Los Angeles and Auckland, and it gives the correct day for both encodings in every zone. It has no offset arithmetic, no `toISOString().substr().split()` chain, and no `Number(tup[...])` re-parse. The shift-then-stringify-then-reparse shape in the PR exists only to make `toISOString` print local fields, which direct getters do for free. The longer-term fix is to make `_on_select` and the Python serializer agree on a single encoding, as the existing `// XXX: this should be handled by the serializer` comment says.

**3. Parameter mutation and comment quality (`date_picker.ts:79-83`).** `date.setTime(...)` mutates the argument of a function that reads like a pure conversion and returns a different `Date`. The three current callers pass temporaries, so nothing breaks today, but it is a trap. The replacement comment lacks the space after `//` used elsewhere in the codebase and contains a typo ("systems's"). It also drops the one thing the old comment explained, that the date arrives as a UTC timestamp while Pikaday works in local time, and never mentions that two encodings reach the function.

**4. The integration test does not guard the regression (`tests/integration/widgets/test_datepicker.py`).** Nothing asserts the initially displayed value, and the initial render from Python is exactly the path Finding 1 breaks. The tests only click day 16 and read back the result, which exercises the one path the PR fixes. Nothing sets `TZ`, so in a UTC CI container old and new code behave identically and the test cannot detect #9129. `max_date=datetime.utcnow()` makes the test depend on the wall clock (it serializes with a time of day and moves with the current date), and the same `DatePicker(...)` construction is repeated three times. There is also no bokehjs unit test, although the logic is a pure function that could be tested across `TZ` values without Selenium. The PR body also leaves the tests and release-note checkboxes unchecked.

## Proposed remediation sequence

1. Replace `_unlocal_date` with an encoding-aware normalizer, using the UTC getters for numbers and a direct parse for `toDateString()` text, and drop the shift and the string round trip. Fix the call sites at lines 68, 70 and 71.
2. Add a bokehjs unit test of that function for UTC, UTC+2, UTC−8 and UTC+12, with both encodings.
3. Add an integration assertion on the initially displayed value for a Python-supplied date. Use a fixed `max_date`, and factor out the shared `DatePicker` construction. If feasible, run the browser under non-UTC `TZ` values.
4. Longer term, unify the stored encoding between `_on_select` and the Python serializer.

## Verification status

Findings 1 and 2 are verified by executing the JS logic under node with `TZ` set (offline, scratch scripts outside the clone). Python serialization was confirmed by reading source. Findings 3 and 4 are verified by reading. Selenium, a browser and the project build were unavailable, so the on-page rendering was not observed.
