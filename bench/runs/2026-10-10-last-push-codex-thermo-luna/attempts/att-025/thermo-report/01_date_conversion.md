# Date conversion: evidence and remediation

## Finding

**UTC-negative clients shift UTC-backed dates back one day.** In `bokehjs/src/lib/models/widgets/date_picker.ts`, `render()` passes `new Date(this.model.value)` to `_unlocal_date` at line 68. The changed lines 82–83 subtract the local offset from every resulting `Date`, then line 85 extracts the adjusted UTC day. That operation assumes the input is local midnight. The Python serializer instead encodes a `datetime.date` as milliseconds from a naive UTC epoch (`bokeh/util/serialization.py:182–184`), so a server-provided 2019-09-20 arrives at UTC midnight. In `America/Los_Angeles`, that instant is local September 19 at 17:00; applying the 420-minute offset moves it to UTC September 19 at 10:00, so `_unlocal_date` returns local September 19. This is a behavioral regression for the default/server value and the min/max boundaries in western timezones.

The adjustment is trying to repair the other representation in the same flow: `_on_select` stores `date.toDateString()` (line 95), which parses back as local midnight. In UTC+ timezones that local midnight has a prior UTC calendar date. The current helper cannot safely apply one formula to both values after `render()` has erased whether the source was a serialized UTC date or a browser-local date string.

## Structural assessment and worked code-judo proposal

The maintenance issue is at the boundary, not the size of the helper. One date-only field is being represented alternately as a UTC-millisecond timestamp and a locale-dependent string, which forces the view to guess what `Date` means from the active timezone. Avoid the guess by retaining one canonical date-only representation. A compact route is to make the browser/server contract an ISO `YYYY-MM-DD` date string and parse its year, month, and day components into the local `Date` Pikaday expects; emit the same canonical form from selection and convert it to Python `date` at the property boundary. If compatibility requires the numeric server timestamp, preserve its origin and build the picker date from its UTC year/month/day components, while parsing selected date strings as local calendar components. Either route removes the offset mutation and keeps calendar dates independent of timezone offsets and DST. Do not infer source semantics from `getTimezoneOffset()` alone.

## Test evidence and coverage gap

`tests/integration/widgets/test_datepicker.py:42–50` checks the label and console only. The JavaScript selection test at lines 52–70 verifies a date after a user click, and the server round-trip at lines 72–99 checks callback timestamps after selecting a date. None asserts that the initial server-supplied September 20 is what Pikaday displays in a western timezone. The test fixture also does not vary timezone. That gap allowed this default-value regression to pass the shape of coverage represented by the new tests.

## Verification status

The project integration suite, Selenium, and a browser are unavailable under this review packet's execution policy, so those tests were not run. A focused Node behavior probe ran the exact changed offset-and-ISO-date logic for `new Date(Date.UTC(2019, 8, 20))` with `TZ=UTC`, `TZ=Europe/Paris`, and `TZ=America/Los_Angeles`. The resulting picker dates were September 20, September 20, and September 19, respectively. The failure is directly reproducible without project dependencies. The checkout was only read; no files in it were changed.
