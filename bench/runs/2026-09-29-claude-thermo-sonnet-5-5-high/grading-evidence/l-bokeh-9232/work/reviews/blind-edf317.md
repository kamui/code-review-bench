# Review blind-edf317

### Item 1
Location: bokehjs/src/lib/models/widgets/date_picker.ts:78-88
Claim: **1. The `_unlocal_date` fix trades a UTC+ bug for a UTC− bug (correctness, blocker).** In `bokehjs/src/lib/models/widgets/date_picker.ts` lines 78–88, `model.value` arrives either as a UTC-midnight millisecond timestamp (what `bokeh/util/serialization.py` emits for a Python `date`) or as a local-midnight string such as "Fri Sep 20 2019" (what `_on_select` writes via `toDateString()`). The original code was right for the first form in all zones and wrong for the second in UTC+ zones, which is issue #9129. The patch subtracts `getTimezoneOffset()` before taking the UTC calendar date, which fixes the second form everywhere and breaks the first in UTC− zones. Running both function bodies under Node with `TZ=America/Los_Angeles` and a UTC-midnight input of 2019-09-20, the old code returned Fri Sep 20 and the new code returns Thu Sep 19. An ISO date string behaves the same way. A Python-supplied `DatePicker(value=date(2019, 9, 20))` therefore renders the wrong day on first paint for US users, and `min_date` and `max_date` shift by a day too, because they use the same helper. The author's testing was in the UK, and the "works in PST" confirmation could only have covered the post-click path. The remedy is to remove the ambiguity instead of correcting for it. Make `_on_select` emit a canonical value that matches what Python sends, such as `Date.UTC(y, m, d)`. Then `_unlocal_date` reduces to `new Date(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate())` with no offset arithmetic. If the emitted format has to stay, branch explicitly on the input type (number versus string) rather than shifting by an offset that suits only one of them. See detail 01, Finding A.
Consequence: —
Fix: —

### Item 2
Location: bokehjs/src/lib/models/widgets/date_picker.ts:78-88
Claim: **2. `_unlocal_date` mutates its argument and does four conversions to compute three integers (boundary and abstraction cleanliness).** Line 83 calls `date.setTime(...)` on the caller's `Date`, so a helper that reads as pure returns a new value and silently edits its input. It then chains `toISOString().substr(0, 10).split('-')` and `Number(...)` calls just to read the year, month and day. All three current callers pass a fresh `Date`, so nothing breaks today, but the next caller will be surprised. Once finding 1 is resolved, the shift should disappear entirely and the string round-trip should be replaced by the `getUTC*` accessors. See detail 01, Finding B.
Consequence: —
Fix: —

### Item 3
Location: bokehjs/src/lib/models/widgets/date_picker.ts:79-81
Claim: **3. The replacement comment is wrong and worse than the one it deleted (legibility).** Lines 79–81 describe `getTimezoneOffset()` as the UTC offset "of date" and claim the result is agnostic to the local system's timezone, which the Node measurements contradict. It also has the typo "systems's" and omits the space after `//` used elsewhere in the file. The deleted comment at least named the real cause, a UTC timestamp read through a local-time representation. Once the code is simplified per finding 1, one accurate sentence saying which encoding the input is in is enough. See detail 01, Finding C.
Consequence: —
Fix: —

### Item 4
Location: tests/integration/widgets/test_datepicker.py:35-98
Claim: **4. The integration tests do not exercise the timezone bug and cannot catch finding 1 (test coverage, blocker for a bug-fix PR).** `tests/integration/widgets/test_datepicker.py` never sets a browser timezone, so on a UTC CI machine the old code and the new code both pass. `test_basic` asserts only the label. Nothing asserts the displayed value on initial render from a Python-supplied date, which is exactly the path finding 1 breaks. `test_js_on_change_executes` asserts the post-click text, but only in the runner's zone. The maintainer floated running under different time zones during the PR discussion and it was not done. Parametrise the display-checking tests over at least one UTC+ zone and one UTC− zone (for example via a driver timezone override), and add an initial-render assertion for `datetime(2019, 9, 20)`. With that in place the regression fails on the UTC− leg. See detail 02, Finding D.
Consequence: —
Fix: —

### Item 5
Location: tests/integration/widgets/test_datepicker.py:35-98
Claim: **5. The test file repeats its fixture and has time-dependent moving parts (maintainability, minor).** The same five-argument `DatePicker(...)` constructor appears three times, and `max_date=datetime.utcnow()` ties the tests to the wall clock for no reason. The server round-trip test builds a plot, source, glyph and custom action only to read two values back. The copyright header still says 2012–2017 for a 2019 file. Extract a factory, use a fixed `max_date`, and trim the round-trip scaffold if the suite conventions allow it. See detail 02, Finding E.
Consequence: —
Fix: —
