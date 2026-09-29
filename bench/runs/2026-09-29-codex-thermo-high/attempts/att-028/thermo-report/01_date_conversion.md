# Date conversion and integration coverage

## Scope and measurements

The change modifies `bokehjs/src/lib/models/widgets/date_picker.ts` (129 lines at the reviewed head) and adds `tests/integration/widgets/test_datepicker.py` (98 lines). The production diff changes only `_unlocal_date`; the new helper logic is three arithmetic/Date operations followed by the existing ISO date extraction. There is no large-file or branching-growth concern here. The maintainability issue is that the helper applies one conversion rule at a boundary that carries two date representations.

The relevant production flow is:

- `render()` constructs `Date` objects from `model.value`, `model.min_date`, and `model.max_date`, then passes each through `_unlocal_date` at lines 68-71.
- `_unlocal_date` adjusts its argument by `getTimezoneOffset()` at lines 82-83, then takes the UTC date components via `toISOString()` at lines 85-87.
- `_on_select` stores `date.toDateString()` at line 95.
- Python's `DatePicker.value`, `min_date`, and `max_date` use the `Date` property (`bokeh/models/widgets/inputs.py:248-267`). The serializer turns Python `date` and `datetime` objects into milliseconds from UTC midnight (`bokeh/util/serialization.py:152-184`).

Thus, the client boundary handles numeric timestamps representing UTC midnight as well as local date strings created by `toDateString()` and parsed back by JavaScript. Those inputs require different normalization. The new offset arithmetic repairs the latter in UTC+ zones, but mutates the already-constructed `Date` and changes the UTC calendar day for timestamp input in west-of-UTC zones. Since `render()` uses the same method for bounds, this is not limited to the selected value.

## Finding: `_unlocal_date` shifts UTC date values in west-of-UTC zones

**Evidence:** `bokehjs/src/lib/models/widgets/date_picker.ts:82-87`, called for `value`, `min_date`, and `max_date` at lines 68-71. The Python `Date` serializer's UTC-midnight timestamp behavior is visible in `bokeh/util/serialization.py:152-184`.

**Problem:** For a date represented as UTC midnight, local offset adjustment does not reconstruct a local date; it changes the UTC date before the method extracts it. A date of 2019-09-20 at UTC midnight is September 19 at 17:00 in `America/Los_Angeles` during daylight time. Subtracting the seven-hour `getTimezoneOffset()` moves that instant to September 19 at 10:00 UTC, and the ISO extraction returns September 19. The previous implementation's ISO extraction returned September 20 for this timestamp. Conversely, a local `toDateString()` parsed by JavaScript represents a local midnight; offset adjustment moves that local midnight to UTC midnight and returns the intended day. A single unconditional correction silently conflates these two inputs.

**Worked code-judo proposal:** Make the date-only representation canonical at the DatePicker boundary, then derive Pikaday's local `Date` from explicit year/month/day components. If compatibility requires accepting both existing forms, make the conversion distinguish the UTC timestamp form from the local string form: preserve the UTC calendar components for UTC-midnight timestamps, and normalize local date strings from local components. Keep this logic in one named conversion function and use it consistently for the value and bounds. This removes the unexplained offset mutation and makes the invariant explicit; it is better than accumulating separate call-site patches for value, min, and max.

**Action:** Replace the unconditional offset mutation with representation-aware normalization (or normalize the model to one date-only representation before this view consumes it). Add cases for initial value and both bounds, covering UTC-midnight timestamps and selected local date strings in UTC+ and UTC− zones.

**Verification status:** Reproduced using Node 24 with the reviewed arithmetic. For input `new Date(Date.UTC(2019, 8, 20))`, output date was September 20 in `UTC` and `Europe/Paris`, but September 19 in `America/Los_Angeles`. For input parsed from `"Fri Sep 20 2019"`, the same arithmetic produced September 20 in both `Europe/Paris` and `America/Los_Angeles`. This confirms the two representations follow different paths. This was a focused behavior check, not the project test suite.

**Command:**

```sh
for zone in UTC Europe/Paris America/Los_Angeles Pacific/Kiritimati; do
  TZ="$zone" node -e 'const d=new Date(Date.UTC(2019,8,20)); const before=d.toISOString().slice(0,10); const ms=d.getTimezoneOffset()*60000; d.setTime(d.getTime()-ms); console.log(process.env.TZ, "timestamp", before, "after", d.toISOString().slice(0,10)); const local=new Date("Fri Sep 20 2019"); const localBefore=local.toISOString().slice(0,10); local.setTime(local.getTime()-local.getTimezoneOffset()*60000); console.log(process.env.TZ, "local-string", localBefore, "after", local.toISOString().slice(0,10));'
done
```

## Finding: integration tests do not validate timezone-sensitive display

**Evidence:** `tests/integration/widgets/test_datepicker.py:42-70` checks the label and a selected JavaScript value. Lines 72-98 verify server callback values after selection. No test controls the browser timezone or asserts the date displayed by the picker for the timezone-sensitive update.

**Problem:** The tests exercise interaction and callback transport, which is useful, but they do not pin the behavior this change is intended to fix. In particular, the browser test does not assert that `value` is initially displayed as September 20, nor does it inspect the rendered input/calendar after the selected value causes a rerender. The server test confirms callback dates but not the user's displayed date. Running these in UTC can pass regardless of the unconditional offset bug; an offset conversion regression affecting display can therefore land while the newly added suite remains green.

**Worked code-judo proposal:** Keep the existing interaction tests, and add one focused regression scenario around the invariant rather than duplicating full server setup for every zone. Make the browser timezone explicit in the test fixture or run a small timezone matrix, load a fixed UTC-midnight date, select another day, and assert both the displayed date and the model value after rerender. Include one west-of-UTC zone to catch the current regression and one east-of-UTC zone to protect the original report. Assert bounds if their display/selection impact is part of the fixture.

**Action:** Add and execute a timezone-controlled integration regression test. It should fail on the reviewed head for a UTC-midnight date in a west-of-UTC zone, and should preserve the selected day after a browser-side update and server round trip.

**Verification status:** Source inspection confirms the added tests have no timezone setup and no post-update rendered-date assertion. The project Selenium suite and browser are explicitly unavailable under this run's policy; no integration test execution was attempted.
