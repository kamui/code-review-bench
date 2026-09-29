# Date normalization

## Finding

In `bokehjs/src/lib/models/widgets/date_picker.ts:82-85`, `_unlocal_date` mutates every input `Date` by subtracting its local offset before reading the ISO date. This treats UTC-midnight timestamps as local-midnight values. Bokeh serializes Python `datetime` and `date` objects as milliseconds from the UTC epoch (`bokeh/util/serialization.py:177-185`), and `render()` passes those values directly to this helper (`date_picker.ts:68-71`). For a September 20 UTC-midnight timestamp in Los Angeles, the original ISO date is September 20, while the new adjustment produces September 19. Normalize each supported input representation according to an explicit date-only contract instead of applying a local offset indiscriminately; ideally convert the model value to a canonical calendar-date representation before constructing Pikaday dates.

## Evidence and analysis

The changed implementation performs these operations in order: obtains `date.getTimezoneOffset()`, subtracts that offset from the same `Date` object, and extracts the resulting UTC ISO date. This compensates for a locally parsed date string such as `Fri Sep 20 2019` in UTC+2: that string parses to local midnight, whose UTC ISO date is September 19, and the adjustment moves it to September 20.

The helper is also called with `new Date(this.model.value)` for `value`, `min_date`, and `max_date`. In the Python serialization layer, both Python `datetime` and Python `date` instances become millisecond counts relative to the UTC epoch. For an input timestamp of `Date.UTC(2019, 8, 20)`, a Los Angeles `Date` has local calendar fields for September 19 and a positive offset of 420 minutes. Subtracting that offset moves it farther back, so the ISO date becomes September 19. The original code’s ISO extraction preserved September 20 for this timestamp. Thus the new generalized offset step fixes one representation while regressing another supported representation in UTC− zones.

The helper also mutates its argument, which hides this normalization side effect in a method named as a date conversion. The current callers pass newly constructed dates, so no additional call-site mutation is evident, but the behavior makes the method contract unnecessarily surprising.

## Code-judo proposal

Remove the attempt to repair mixed representations by shifting a `Date` and then parsing its ISO string. First establish a canonical date-only representation at the model boundary, preserving whether incoming values mean a UTC calendar date or a local calendar date string. Then create the local `Date` Pikaday needs from the canonical year, month, and day. If compatibility requires both numeric timestamps and strings, make their interpretation explicit at that boundary and test each path. This removes the mutating time arithmetic and the hidden assumption that every input `Date` came from local midnight, while keeping timezone handling out of Pikaday setup.

## Verification

A scratch Node check compared the old ISO extraction with the changed offset adjustment for September 20 in `UTC`, `Europe/Paris`, and `America/Los_Angeles`. For the UTC-midnight timestamp, the old extraction returned `2019-09-20` in all three zones; the changed adjustment returned `2019-09-19` in Los Angeles. For a locally parsed September 20 date string, the old extraction returned September 19 in Paris and the changed adjustment returned September 20, confirming the targeted case. The test suite and browser are unavailable per the packet; no integration test was run.

## Action

Define and document the calendar-date contract for `value`, `min_date`, and `max_date`; implement a non-mutating conversion that handles each accepted input representation explicitly; then cover UTC+ string and UTC− timestamp cases before merging.
