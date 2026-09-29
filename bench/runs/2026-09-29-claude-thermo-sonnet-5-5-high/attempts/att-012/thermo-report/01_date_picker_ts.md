# Detail 01 — bokehjs `date_picker.ts` (`_unlocal_date`)

Scope: `bokehjs/src/lib/models/widgets/date_picker.ts`, lines 78-88 (+6/-2). Range `main...review-head`.

## Finding A — the offset shim fixes local-midnight inputs but breaks UTC-midnight inputs in UTC- zones (verified)

`DatePickerView.render()` feeds `_unlocal_date` values from `this.model.value`, `min_date` and `max_date` (lines 68-71). Those values arrive in two incompatible shapes:

1. Values set from Python. `Date` values are serialized by `convert_datetime_type` (`bokeh/util/serialization.py:183-184`) as ms since epoch at UTC midnight. ISO date-only strings parse the same way in JS, as UTC midnight.
2. Values written back by the widget itself. `_on_select` (line 95) stores `date.toDateString()`, e.g. "Mon Sep 16 2019". `new Date()` parses that string as local midnight.

The old code (`toISOString().substr(0,10)`) was correct for shape 1 everywhere and wrong for shape 2 in UTC+ zones. That is issue #9129. The new code subtracts `getTimezoneOffset()` first. That is correct for shape 2 everywhere, but it moves shape 1 back a day in every UTC- zone.

Measured with node under scratch scripts in the work directory. The script is `t.js` in the attempt tmp dir. It ran `Date.UTC(2019,8,20)`, "2019-09-20" and "Fri Sep 20 2019" through the old and new functions:

| TZ | input | old result | new result |
| --- | --- | --- | --- |
| Europe/Paris | ms / ISO date | Fri Sep 20 | Fri Sep 20 |
| Europe/Paris | toDateString | Thu Sep 19 (bug) | Fri Sep 20 |
| America/Los_Angeles | ms / ISO date | Fri Sep 20 | **Thu Sep 19 (new regression)** |
| America/Los_Angeles | toDateString | Fri Sep 20 | Fri Sep 20 |
| UTC, Pacific/Auckland | all | correct (Auckland toDateString old: wrong) | correct |

Consequence: in the US, `DatePicker(value=date(2019, 9, 20))` now renders 19 Sep on first load. Its `min_date` and `max_date` are shifted a day too, so the allowed range is wrong at both ends. The bug moves from "after a click, in UTC+" to "on initial load, in UTC-", which is the most common path. The fix works only when the value has been through `toDateString`. Verification status: reproduced with node under TZ; the browser suite was unavailable.

Remedy: fix the ambiguity at its source rather than compensating in the display path (see the code-judo section).

## Finding B — the helper mutates its argument and takes a needless string round trip (verified by reading)

`date.setTime(...)` mutates the parameter of a function that is named like a pure converter. It is safe today only because every caller passes a fresh `new Date(...)`. The function then does `toISOString()`, `substr`, `split`, `Number()` x3 and `new Date(y, m, d)`. Where the code is not simple, this is a lot of ceremony to read three fields. The comment dropped the "this sucks" explanation of *why* two representations exist and replaced it with prose about arithmetic ("multiply to get the offset in ms"). The reader now learns the mechanics but not the invariant, which is why Finding A slipped in. Comments also lack a space after `//` and have a typo ("systems's"), unlike the surrounding code.

## Code-judo proposal

The real defect is that `model.value` has no single canonical meaning: sometimes UTC midnight, sometimes local midnight. Two options that delete the special cases:

1. Parse by shape at one boundary. Write one `parse_date(value): Date` that returns a local-midnight `Date`. For a string matching `/^\d{4}-\d{2}-\d{2}$/` or for a number, build it from `getUTC*` fields. For anything else (the `toDateString` form), `new Date(value)` is already local midnight and is returned as-is. `render()` calls it three times and `_unlocal_date` disappears.
2. Make `_on_select` write the same shape Python writes. For example, `Date.UTC(y, m, d)` from `date.getFullYear()/getMonth()/getDate()`. Then everything on the model is UTC midnight and `_unlocal_date` is simply `new Date(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate())` with no offset arithmetic. This changes the wire format (the `toDateString()` comment cites #4965 and #7048), so option 1 is the safer step.

Either removes the mutation, the string round trip and the DST-sensitive `getTimezoneOffset()` use (its value depends on the instant being converted, so near DST changes it can differ from the offset at local midnight).
