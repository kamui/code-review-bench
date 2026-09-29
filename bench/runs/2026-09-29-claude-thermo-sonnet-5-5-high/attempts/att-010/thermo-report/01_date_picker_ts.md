# Detail 01 — `bokehjs/src/lib/models/widgets/date_picker.ts`

Scope: the +6/−2 change to `DatePickerView._unlocal_date` (lines 78–88 at head `36549bca3`).

## What the diff does

`_unlocal_date(date)` converts the `Date` built from `model.value` into a local-midnight `Date` for Pikaday. The old body took the UTC calendar date via `toISOString()` and rebuilt a local `Date` from its parts. The patch first shifts the timestamp by `getTimezoneOffset() * 60000` (mutating the argument with `setTime`), then runs the old body unchanged.

## Finding A — the fix trades a UTC+ bug for a UTC− bug (correctness / wrong model)

`model.value` reaches `new Date(this.model.value)` in two incompatible representations:

1. A UTC-midnight millisecond timestamp when the value comes from Python. `bokeh/util/serialization.py:183-184` converts a `date` to `(datetime(...) - DT_EPOCH).total_seconds() * 1000`, and `Date.transform` in `bokeh/core/property/datetime.py` also accepts ISO strings, which JS parses as UTC.
2. A local-midnight string such as `"Fri Sep 20 2019"`, which `_on_select` writes with `date.toDateString()` (line 95). JS parses this as local time.

The old code was right for (1) in every zone and wrong for (2) in UTC+ zones. That is issue #9129. The patch is right for (2) in every zone and wrong for (1) in UTC− zones.

Measured with scratch Node (`TZ=<zone> node t.js`, the old and new bodies copied verbatim, input `2019-09-20`):

| TZ | input | old result | new result |
| --- | --- | --- | --- |
| America/Los_Angeles | UTC ms | Fri Sep 20 | **Thu Sep 19** |
| America/Los_Angeles | ISO string | Fri Sep 20 | **Thu Sep 19** |
| America/Los_Angeles | local string | Fri Sep 20 | Fri Sep 20 |
| Europe/Paris | UTC ms | Fri Sep 20 | Fri Sep 20 |
| Europe/Paris | local string | **Thu Sep 19** | Fri Sep 20 |
| Pacific/Auckland, Pacific/Kiritimati | local string | **Thu Sep 19** | Fri Sep 20 |
| UTC | all | correct | correct |

So a Python `DatePicker(value=date(2019, 9, 20))` rendered for an American user now displays Sep 19 on first paint. The same happens for `min_date` and `max_date`, which use the same helper, so the selectable range is shifted a day too. Before the patch that user saw the right day. The author confirmed the fix only from a UTC+ zone. The maintainer's "works great in PST" comment could only have been about the post-click path, because the initial-render path is what breaks.

This is not a subtle edge case. It is a symmetric regression on the primary data path of the widget (server-provided values) for a very large user population.

### Code-judo proposal

The bug exists because one property carries two encodings and the helper guesses which one it got. Remove the ambiguity instead of adding an offset correction:

- Make `_on_select` write a canonical value. For example, write the UTC-midnight timestamp `Date.UTC(y, m, d)`, or a `YYYY-MM-DD` string built from local parts. That matches what Python sends and what `Date.transform` accepts.
- Then `_unlocal_date` becomes a one-line, zone-independent function: read `getUTCFullYear/Month/Date` and return `new Date(y, m, d)`. This is the original body without the `toISOString().substr().split('-')` string round-trip, and it needs no offset arithmetic, no mutation and no branching.
- If changing the emitted format is too disruptive (`toDateString()` is deliberately used, see comment at line 91-94), the fallback is to branch explicitly on `typeof value` (number → UTC parts, string → `new Date(value)` local parts). That is an honest, testable statement of the two inputs and is still better than a shift that is correct for only one of them.

## Finding B — helper mutates its argument and hides the real transform (boundary cleanliness)

`date.setTime(...)` at line 83 mutates the caller's `Date`. Today all three call sites pass a fresh `new Date(...)`, so nothing breaks. A function named `_unlocal_date` that returns a new `Date` and also silently edits its input is a trap for the next caller. The shift-then-`toISOString`-then-`substr`-then-`split`-then-`new Date(...)` pipeline is four conversions to compute three integers. Once the shift is dropped (Finding A), the string round-trip should go too: use `getUTCFullYear()`, `getUTCMonth()`, `getUTCDate()`.

## Finding C — comment is inaccurate and stylistically off (legibility, minor)

The comment at lines 79–81 says "Get the UTC offset … of date", but `getTimezoneOffset()` returns the offset of the *local system* at that instant. It claims the result is "agnostic to their local systems's timezone", which the measurements above contradict. It has a typo (`systems's`) and lacks the space after `//` that the surrounding file uses. The deleted comment at least named the real problem (UTC timestamp versus local representation). The replacement describes the mechanism and asserts a property that does not hold. A wrong comment is worse than none.

## Other notes

- Verified: `getTimezoneOffset()` is evaluated on the pre-shift instant, so there is a DST-boundary nuance. Around a DST change the pre-shift offset could differ from the offset at local midnight. This is minor next to Finding A and not measured.
- No file-size issue: `date_picker.ts` is 130 lines.
- The PR body leaves "tests added / passed" and the release-note box unchecked. Tests were added in the same PR.

## Verification status

Finding A: reproduced under Node with `TZ` set, offline, by copying both function bodies verbatim. Scratch script `t.js` under the run's temp directory. Serialization path confirmed by reading `bokeh/util/serialization.py` and `bokeh/core/property/datetime.py`. The browser and Selenium suite were unavailable, so the end-to-end render was not run. Findings B and C are by inspection.
