# 01 — `bokehjs/src/lib/models/widgets/date_picker.ts`

Review range: `ccb4bcb4c2b841d89b0e88303a97bf4604a5795f..36549bca3a63d581f7b68d08054a7813c1e6a499` (`git diff main...review-head`).
File size after the PR: 129 lines (`wc -l`), +6/−2 in this PR. There is no file-size concern.

## Background: what `model.value` actually holds

Reading the head file together with the Python model shows that the view receives
`value`, `min_date` and `max_date` in **two different shapes**, and they encode the
calendar day in two different time zones:

1. **From Python.** `DatePicker.value/min_date/max_date` are `Date` properties
   (`bokeh/models/widgets/inputs.py:253-263`). They are serialized by
   `convert_datetime_type` (`bokeh/util/serialization.py:176-183`) as float
   milliseconds since the epoch, computed against a naive `DT_EPOCH`. A
   `date(2019, 9, 20)` therefore arrives as `Date.UTC(2019, 8, 20)`, which is
   **midnight UTC**. A `datetime` such as `datetime.utcnow()`, which the new tests use
   for `max_date`, arrives as a UTC wall-clock instant with a time-of-day component.
2. **From the browser.** `_on_select` (lines 90-96) writes `date.toDateString()`, for
   example `"Mon Sep 16 2019"`. The built-in default at line 124 is
   `new Date().toDateString()`. `new Date("Mon Sep 16 2019")` parses that string as
   **local midnight**.

The TypeScript contract hides this: `value: p.Property<string>` (lines 104-106) is
declared over `p.Any` with a `// TODO (bev) types` comment (lines 123-126). In
practice the property carries either a number or a string, and no type tells a reader
which time zone applies.

Before the PR, `_unlocal_date` read the calendar day in UTC
(`toISOString().substr(0, 10)`). That was right for shape 1 and wrong for shape 2 in
UTC+ zones, which is issue #9129. The PR changes the reader to take the day in local
time. That is right for shape 2 and wrong for shape 1 in UTC− zones.

## Finding 1.1 — The offset shift moves the off-by-one bug from UTC+ zones to UTC− zones instead of fixing it (lines 82-83)

**Status: CONFIRMED by execution** (node v24, scratch script, several `TZ` values).

The two new lines are:

```ts
const timeOffsetInMS = date.getTimezoneOffset() * 60000
date.setTime(date.getTime() - timeOffsetInMS)
```

They shift the instant so that its UTC fields equal its local fields. Every value then
has its calendar day read in the browser's local zone. For a value set from Python,
which is UTC midnight, any zone west of Greenwich now resolves to the previous day.
`render()` sends `value`, `min_date` and `max_date` through this helper (lines 68-71),
so all three are affected:

- In America, a user whose app sets `DatePicker(value=date(2019, 9, 20))` now sees Sep 19
  highlighted and pre-filled. Before the PR they saw Sep 20 correctly.
- `min_date=date(2019, 9, 1)` now allows Aug 31, and `max_date` moves back one day.
- Pikaday calls `setDate(defaultDate, true)` for the default without firing `onSelect`,
  so the model value stays correct while the UI shows the wrong day. This is the same
  mismatch between shown and stored values that #9129 reports, just in the other
  hemisphere.

Command (script at `clone-work/scratch/unlocal.js`; it copies the old and new
`_unlocal_date` bodies verbatim, apart from the TypeScript annotations):

```
for tz in UTC Europe/Paris Europe/London America/Los_Angeles Pacific/Kiritimati Pacific/Pago_Pago; do TZ=$tz node unlocal.js; done
```

Relevant output rows:

```
TZ=America/Los_Angeles
  python date(2019,9,20) -> UTC-midnight ms: old=Fri Sep 20 2019 new=Thu Sep 19 2019 ...
  JS select -> "Mon Sep 16 2019" string:     old=Mon Sep 16 2019 new=Mon Sep 16 2019 ...
TZ=Pacific/Pago_Pago
  python date(2019,9,20) -> UTC-midnight ms: old=Fri Sep 20 2019 new=Thu Sep 19 2019 ...
TZ=Europe/Paris
  python date(2019,9,20) -> UTC-midnight ms: old=Fri Sep 20 2019 new=Fri Sep 20 2019 ...
  JS select -> "Mon Sep 16 2019" string:     old=Sun Sep 15 2019 new=Mon Sep 16 2019 ...
TZ=UTC
  (old == new for every input)
```

The PR discussion mentions a manual check in PST. That check covered the select-then-redisplay
path, which the PR does fix everywhere. It did not cover the initial display of a
Python-provided date, and that path is where the regression appears. The new integration
tests do not assert on the initial display either (see `02_integration_tests.md`).

**Root cause.** The helper is being asked to guess the time zone of an input whose
shape it never inspects. Changing the guess from "UTC" to "local" only swaps which half
of the world gets the wrong day. See Finding 1.2 for the structural fix.

## Finding 1.2 — Missed code-judo: normalize the value where it is written, and the offset arithmetic disappears (lines 90-96, 104-106, 123-126)

**Status: proposal verified by execution** (scratch `judo.js`, all zones listed below).

The bug comes from one property carrying two encodings that use different time zones.
The PR adds offset arithmetic on the read side. The simpler move is to stop producing the
second encoding at all. `_on_select` should write a time-zone-free ISO calendar date built
from Pikaday's local fields. `new Date("YYYY-MM-DD")` is defined by ECMAScript to parse as
UTC midnight, so after this change **every** value, whether it came from Python or from
the browser, is a UTC-midnight date. The original UTC-field reader is then correct for all
inputs with no `getTimezoneOffset` at all.

Worked proposal A (writer-side, preferred):

```ts
// value/min_date/max_date always denote a calendar day at UTC midnight:
// Python sends ms-since-epoch, and _on_select sends "YYYY-MM-DD" (parsed as UTC).
function to_picker_date(value: string | number): Date {
  const d = new Date(value)
  return new Date(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate())
}

function to_iso_date(date: Date): string {
  const pad = (n: number) => `${n}`.padStart(2, "0")   // or a core/util string helper if one exists
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

// render():
defaultDate: to_picker_date(this.model.value),
minDate: this.model.min_date != null ? to_picker_date(this.model.min_date) : undefined,
maxDate: this.model.max_date != null ? to_picker_date(this.model.max_date) : undefined,

// _on_select():
this.model.value = to_iso_date(date)

// defaults:
value: [ p.Any, to_iso_date(new Date()) ],
```

This removes the timezone-offset arithmetic, the in-place mutation, the
ISO-string→split→`Number()` round trip, and the misleading "toISOString returns the wrong
day" warning in `_on_select`. That warning only applies to `toISOString()` of a
*local-midnight* instant. `to_iso_date` never builds one. On the Python side, `Date.transform`
(`bokeh/core/property/datetime.py:66-67`) already runs `dateutil.parser.parse(value).date()`
on strings, so `"2019-09-16"` round-trips without any Python change. The typed signature
`value: string | number` also makes the real boundary explicit, replacing the
`p.Property<string>` claim.

One trade-off needs a decision. `cb_obj.value` in `CustomJS` callbacks changes from
`"Mon Sep 16 2019"` to `"2019-09-16"`, and the new `test_js_on_change_executes` asserts
the old format. If maintainers treat that string format as public, use proposal B.

Proposal B (reader-side, keeps the wire format). Branch once on the shape that actually
carries the time-zone information:

```ts
// Python sends ms-since-epoch at UTC midnight; _on_select sends local toDateString().
function to_picker_date(value: string | number): Date {
  const d = new Date(value)
  return typeof value == "number"
    ? new Date(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate())
    : new Date(d.getFullYear(), d.getMonth(), d.getDate())
}
```

Verification command:

```
for tz in UTC Europe/Paris America/Los_Angeles Pacific/Kiritimati Pacific/Pago_Pago; do TZ=$tz node judo.js; done
```

Output (the Python value is `Date.UTC(2019,8,20)`; the picked day is local Sep 16):

```
UTC                  A: py-> Fri Sep 20 2019 | pick-> 2019-09-16 -> Mon Sep 16 2019 || B: py-> Fri Sep 20 2019 | pick-> Mon Sep 16 2019
Europe/Paris         A: py-> Fri Sep 20 2019 | pick-> 2019-09-16 -> Mon Sep 16 2019 || B: py-> Fri Sep 20 2019 | pick-> Mon Sep 16 2019
America/Los_Angeles  A: py-> Fri Sep 20 2019 | pick-> 2019-09-16 -> Mon Sep 16 2019 || B: py-> Fri Sep 20 2019 | pick-> Mon Sep 16 2019
Pacific/Kiritimati   A: py-> Fri Sep 20 2019 | pick-> 2019-09-16 -> Mon Sep 16 2019 || B: py-> Fri Sep 20 2019 | pick-> Mon Sep 16 2019
Pacific/Pago_Pago    A: py-> Fri Sep 20 2019 | pick-> 2019-09-16 -> Mon Sep 16 2019 || B: py-> Fri Sep 20 2019 | pick-> Mon Sep 16 2019
```

Both proposals give the correct day for both sources in every zone tested. The PR as
written does not (Finding 1.1).

Caveat, not verified: a Python `datetime` with a non-midnight time (for example
`max_date=datetime.utcnow()`) still rounds by its UTC day under A and B. That is the
pre-PR semantics and arguably correct for a `Date` property. The cleaner end state is
for `Date.transform` to truncate `datetime` to `date`, but that is outside this diff.

## Finding 1.3 — `_unlocal_date` is now a convoluted, argument-mutating way to spell "take the local calendar fields", under a name and comment that describe the opposite (lines 78-88)

**Status: CONFIRMED by execution.** In every zone and for every input, `new` equals
`new Date(d.getFullYear(), d.getMonth(), d.getDate())` (the `localFields` column), and
`arg_mutated=true` in every non-UTC zone.

After the change, the helper shifts the instant by `getTimezoneOffset()`, serializes it
to an ISO string, slices out the first ten characters, splits on `-`, converts to numbers,
subtracts one from the month, and rebuilds a `Date`. Algebraically that is
`new Date(d.getFullYear(), d.getMonth(), d.getDate())`, and running the code confirms it.
So the helper now:

- **Lies about what it does.** The method is called `_unlocal_date`, but it now
  *localizes*: it keeps the local calendar day. The removed comment explained the
  UTC-vs-local tension. The new comment says the result is "agnostic to their local
  systems's timezone", which is false (Finding 1.1 shows the output depends on the zone).
  A future maintainer who trusts either the name or the comment will be misled.
- **Mutates its argument** through `date.setTime(...)`. All three current callers pass a
  fresh `new Date(...)`, so nothing breaks today. But a helper named like a pure
  conversion that silently rewrites its input is a trap for the next caller, and there
  is no reason for it: the shift could be done on a copy, or better, not at all.
- **Adds indirection without earning it.** A string round trip that only exists to read
  date fields is the "magic" style that makes this file hard to reason about. The
  direct field-read shown in Finding 1.2 is shorter and says what it means.

Remedy: replace the helper with the explicit `to_picker_date` from Finding 1.2 (a pure,
module-level function that takes `string | number`). If the maintainers deliberately want
local semantics for every input, which Finding 1.1 argues against, then it should at
least be written as the one-line local-fields constructor, without mutation, and renamed
with an accurate comment.

## Non-findings checked

- The file size is fine (129 lines).
- Hoisting `timeOffsetInMS` would not help, because the offset is per-instant (DST) and
  must be computed from `date`. That is correct as written; the issue is the model, not
  the arithmetic.
- The `.idea/vcs.xml` addition from commit `7aae927cf` is removed again in `e92066d59`. The
  net diff does not include it.
