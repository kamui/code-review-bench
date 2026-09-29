# Detail 01 — `bokehjs/src/lib/models/widgets/date_picker.ts`

Range reviewed: `main...review-head` (merge-base `ccb4bcb4c`, head `36549bca3`). The production change is +6/−2 lines inside `DatePickerView._unlocal_date`.

## Background established from the source

`DatePickerView.render()` (date_picker.ts:57-76) feeds `this.model.value`, `min_date` and `max_date` through `_unlocal_date(new Date(x))` before handing them to Pikaday. Two different producers write `model.value`:

1. Python. `bokeh/core/json_encoder.py` -> `convert_datetime_type` (`bokeh/util/serialization.py:183-184`) serializes a `datetime.date` as milliseconds since epoch at **UTC midnight**. `new Date(ms)` on this value is a UTC-midnight instant. `min_date` and `max_date` arrive the same way.
2. The widget itself. `_on_select` (date_picker.ts:90-97) writes `date.toDateString()`, e.g. `"Mon Sep 16 2019"`. `new Date("Mon Sep 16 2019")` parses as **local midnight**.

`_unlocal_date` therefore receives two different instants for the same calendar day, and the correct extraction differs: UTC fields for (1), local fields for (2). The original code took UTC fields (correct for 1, wrong for 2 in UTC+ zones, which is issue #9129). The PR shifts the instant by the local offset (date_picker.ts:82-83), which takes local fields (correct for 2, wrong for 1 in UTC- zones).

## Measurements

Scratch script (`att-011/tmp/t.js`, run offline with node, no clone changes) copies the old and new function bodies and applies them to (a) `Date.UTC(2019,8,20)` (what Python sends for 2019-09-20) and (b) `new Date("Mon Sep 16 2019")` (what a pick stores). Command: `TZ=<zone> node t.js`. Output rendered with `toDateString()`:

| TZ | Python-sent 2019-09-20, original | same, PR | picked "Mon Sep 16 2019", original | same, PR |
| --- | --- | --- | --- | --- |
| UTC | Fri Sep 20 | Fri Sep 20 | Mon Sep 16 | Mon Sep 16 |
| Europe/London | Fri Sep 20 | Fri Sep 20 | **Sun Sep 15** | Mon Sep 16 |
| Europe/Paris | Fri Sep 20 | Fri Sep 20 | **Sun Sep 15** | Mon Sep 16 |
| Asia/Kolkata | Fri Sep 20 | Fri Sep 20 | **Sun Sep 15** | Mon Sep 16 |
| Pacific/Auckland | Fri Sep 20 | Fri Sep 20 | **Sun Sep 15** | Mon Sep 16 |
| America/Los_Angeles | Fri Sep 20 | **Thu Sep 19** | Mon Sep 16 | Mon Sep 16 |
| America/Sao_Paulo | Fri Sep 20 | **Thu Sep 19** | Mon Sep 16 | Mon Sep 16 |

The PR fixes the picked-value column for UTC+ zones and breaks the Python-sent column for UTC- zones. Status: **verified by execution** (node v24, scratch code only).

## Finding 1 — the fix trades the UTC+ bug for a mirror-image UTC- bug (verified)

`_unlocal_date` is still one function trying to serve two inputs with different meaning, and the PR moves the wrong-answer region from UTC+ to UTC- rather than removing it. Anyone in the Americas who builds `DatePicker(value=date(2019, 9, 20))` now sees the widget open on 19 Sep, and the same one-day error hits `min_date`/`max_date` (date_picker.ts:70-71), so the first selectable day and the last selectable day shift as well. The maintainers' PST test ("working great for me in PST") most likely exercised only the pick-then-re-render path, which is the path that is right for UTC-. The initial-render path was not examined.

The remedy is not another offset tweak; see the code-judo proposal below.

## Finding 2 — `_unlocal_date` mutates its argument and round-trips through an ISO string (verified by reading)

After the change the function does `date.setTime(...)` on the parameter (date_picker.ts:83), then formats to an ISO string, slices, splits, re-parses into numbers and builds a new `Date`. Mutating a caller-supplied `Date` is a hidden side effect (callers today pass throwaway `new Date(...)` values, which is the only reason it is harmless). The string round-trip exists only to read three calendar fields; `getUTCFullYear()/getUTCMonth()/getUTCDate()` read them directly. The three-line `//` comment blocks (date_picker.ts:79-81) describe the arithmetic mechanism, not the actual invariant (two input conventions), which is why the UTC- regression was not noticed. Comment style also deviates from the file (`//Get` without a space, "systems's").

## Code-judo proposal

Delete the ambiguity instead of compensating for it. The widget owns exactly one of the two producers, so make it emit the same convention as Python. Concretely:

- In `_on_select`, store the picked day as UTC-midnight, e.g. `Date.UTC(date.getFullYear(), date.getMonth(), date.getDate())` (a number, like Python's serialization; `value` is `p.Any`), keeping the websocket-safe non-`toString()` property of the existing comment. The displayed text in the input is produced by Pikaday itself, so `.bk-input` text is unaffected.
- `_unlocal_date` then has a single input meaning and reduces to `new Date(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate())` with no offset arithmetic, no mutation and no string round trip. It is correct in every zone by construction, which the original ISO-slice was for Python-sent values.
- The default `new Date().toDateString()` in `init_DatePicker` (date_picker.ts:124) is also a local-midnight string and would need the same treatment (e.g. a UTC-midnight default) so that the single-convention invariant holds.

If changing the stored format is judged too invasive, the smaller alternative is to normalize at the top of `_unlocal_date` by an explicit type check (number => UTC fields, string => local fields), with a comment naming the two producers. That still adds a branch, so the single-convention route is preferred.

## Remediation order

1. Decide the single canonical convention for `model.value` (recommend UTC-midnight ms).
2. Rewrite `_on_select`, the default value and `_unlocal_date` accordingly; drop the mutation and the ISO string round trip.
3. Add the zone-varying regression test described in detail file 02.
