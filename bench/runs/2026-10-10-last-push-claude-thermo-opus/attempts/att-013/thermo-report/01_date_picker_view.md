# 01 — `DatePickerView` date conversion

Subsystem: `bokehjs/src/lib/models/widgets/date_picker.ts` (129 lines at the head, +6 / −2 in this PR).
Carries the evidence for findings 1 and 2 of `summary.md`.

## What the PR changed

The whole production change is inside `_unlocal_date` (lines 78–88 at the head):

```ts
  _unlocal_date(date: Date): Date {
    //Get the UTC offset (in minutes) of date (will be based on the timezone of the user's system).
    //Then multiply to get the offset in ms.
    //This way it can be used to recreate the user specified date, agnostic to their local systems's timezone.
    const timeOffsetInMS = date.getTimezoneOffset() * 60000
    date.setTime(date.getTime() - timeOffsetInMS)

    const datestr = date.toISOString().substr(0, 10)
    const tup = datestr.split('-')
    return new Date(Number(tup[0]), Number(tup[1])-1, Number(tup[2]))
  }
```

The two `const`/`setTime` lines are new, and the old two-line comment ("the date comes in as a
UTC timestamp and pikaday uses Date's local timezone-converted representation") was replaced by
the three-line one above.

The helper has three callers, all in `render()` (lines 68, 70, 71): `defaultDate`, `minDate` and
`maxDate` of the Pikaday options, each as `this._unlocal_date(new Date(this.model.<prop>))`.

## The two encodings that reach the helper

Reading the code on both sides of the wire, `model.value` arrives in one of two shapes.

The first is a number. On the Python side `DatePicker.value`, `min_date` and `max_date` are
`Date` properties (`bokeh/models/widgets/inputs.py`). `Property.prepare_value`
(`bokeh/core/property/bases.py` lines 317–329) runs `Date.transform`
(`bokeh/core/property/datetime.py` lines 58–68), which turns strings and numbers into
`datetime.date` and lets `date`/`datetime` objects through. The JSON encoder then sends them
through `convert_datetime_type` (`bokeh/util/serialization.py` lines 177–184), which subtracts
`DT_EPOCH = utcfromtimestamp(0)` from the naive value. So `date(2019, 9, 20)` crosses the wire as
`1568937600000`: the instant whose **UTC** fields read 2019‑09‑20 00:00. The calendar day the
Python user wrote is in the UTC fields, in every browser timezone.

The second is a string. `_on_select` (line 95) assigns `date.toDateString()`, for example
`"Mon Sep 16 2019"`. `model.change` then re-runs `render()`, and `new Date("Mon Sep 16 2019")`
parses that form as **local** midnight. The calendar day is in the local fields.

`min_date` and `max_date` are only ever written from Python, so they are always the first shape.

The merge-base helper read the UTC fields, which is right for the number and wrong for the string
east of Greenwich (issue #9129). The head helper reads what amounts to the local fields, which is
right for the string and wrong for the number west of Greenwich.

## Measurement 1 — timezone matrix (verified by execution)

Scratch script `clone-work/scratch/tz_matrix.js` copies the merge-base body (`old`), the head body
(`new`) and the proposed replacement (`prop`), and feeds each the Python-origin number for
`date(2019, 9, 20)` and the JS-origin string `"Mon Sep 16 2019"`.

Command:

```
for tz in UTC Europe/Paris Europe/London Asia/Kolkata Pacific/Kiritimati \
          America/Los_Angeles America/New_York America/Sao_Paulo Pacific/Pago_Pago; do
  TZ=$tz node tz_matrix.js
done
```

Output (node v24.21.0), reformatted into a table:

| TZ | Python `date(2019,9,20)` — old | new (head) | proposed | JS `"Mon Sep 16 2019"` — old | new (head) | proposed |
| --- | --- | --- | --- | --- | --- | --- |
| UTC | Fri Sep 20 | Fri Sep 20 | Fri Sep 20 | Mon Sep 16 | Mon Sep 16 | Mon Sep 16 |
| Europe/Paris | Fri Sep 20 | Fri Sep 20 | Fri Sep 20 | **Sun Sep 15** | Mon Sep 16 | Mon Sep 16 |
| Europe/London | Fri Sep 20 | Fri Sep 20 | Fri Sep 20 | **Sun Sep 15** | Mon Sep 16 | Mon Sep 16 |
| Asia/Kolkata | Fri Sep 20 | Fri Sep 20 | Fri Sep 20 | **Sun Sep 15** | Mon Sep 16 | Mon Sep 16 |
| Pacific/Kiritimati | Fri Sep 20 | Fri Sep 20 | Fri Sep 20 | **Sun Sep 15** | Mon Sep 16 | Mon Sep 16 |
| America/Los_Angeles | Fri Sep 20 | **Thu Sep 19** | Fri Sep 20 | Mon Sep 16 | Mon Sep 16 | Mon Sep 16 |
| America/New_York | Fri Sep 20 | **Thu Sep 19** | Fri Sep 20 | Mon Sep 16 | Mon Sep 16 | Mon Sep 16 |
| America/Sao_Paulo | Fri Sep 20 | **Thu Sep 19** | Fri Sep 20 | Mon Sep 16 | Mon Sep 16 | Mon Sep 16 |
| Pacific/Pago_Pago | Fri Sep 20 | **Thu Sep 19** | Fri Sep 20 | Mon Sep 16 | Mon Sep 16 | Mon Sep 16 |

The bold cells are the wrong days. The merge-base has four, all in the string column east of
Greenwich. The head has four, all in the number column west of Greenwich. The proposed function
has none.

## Measurement 2 — bounds (verified by execution)

Scratch script `clone-work/scratch/minmax.js` feeds the numbers for `min_date=date(2019, 9, 1)`
and `max_date=date(2019, 9, 30)` through both bodies.

```
UTC                    min old: Sun Sep 01 2019 new: Sun Sep 01 2019 | max old: Mon Sep 30 2019 new: Mon Sep 30 2019
Europe/Paris           min old: Sun Sep 01 2019 new: Sun Sep 01 2019 | max old: Mon Sep 30 2019 new: Mon Sep 30 2019
America/Los_Angeles    min old: Sun Sep 01 2019 new: Sat Aug 31 2019 | max old: Mon Sep 30 2019 new: Sun Sep 29 2019
```

At the head, a Los Angeles browser gets a selectable window of Aug 31 – Sep 29 for a declared
window of Sep 1 – Sep 30.

## Measurement 3 — the head body is local start-of-day (verified by execution)

The local wall-clock fields of an instant `t` are the UTC fields of `t − offset(t)`. The head body
computes exactly `t − offset(t)`, reads the UTC date out of the ISO string, and builds a local
`Date` from it. That is `new Date(d.getFullYear(), d.getMonth(), d.getDate())` written the long
way round.

Scratch script `clone-work/scratch/equiv.js` compares the two for every 37th minute from
2015‑01‑01 to 2025‑01‑01, and separately records how far the argument is moved.

```
UTC                    instants: 142171 mismatches vs local start-of-day: 0 | arg mutated by ms: 0
Europe/Paris           instants: 142171 mismatches vs local start-of-day: 0 | arg mutated by ms: 7200000
Asia/Kolkata           instants: 142171 mismatches vs local start-of-day: 0 | arg mutated by ms: 19800000
Australia/Lord_Howe    instants: 142171 mismatches vs local start-of-day: 0 | arg mutated by ms: 37800000
America/Los_Angeles    instants: 142171 mismatches vs local start-of-day: 0 | arg mutated by ms: -25200000
America/Sao_Paulo      instants: 142171 mismatches vs local start-of-day: 0 | arg mutated by ms: -10800000
America/St_Johns       instants: 142171 mismatches vs local start-of-day: 0 | arg mutated by ms: -9000000
Pacific/Apia           instants: 142171 mismatches vs local start-of-day: 0 | arg mutated by ms: 46800000
```

Zero mismatches in all eight zones, which include half-hour offsets (Kolkata, St Johns, Lord Howe)
and every daylight-saving transition in the decade. The last column shows the `setTime` side
effect on the caller's `Date`.

## Why this is a structural problem and not only a bug

The function's name, its comment and its body now disagree three ways. The name says "un-local".
The comment says the result is "agnostic to their local systems's timezone". The body returns the
local day, which is the most timezone-dependent answer available. The comment that was deleted
was the only statement in the file of what the input actually is.

The arithmetic also hides a simple data-shape assumption behind a generic-looking mechanism,
which is the pattern the review standard calls out. There is no single offset correction that is
right for both encodings: one needs zero correction and the other needs the full offset. Any body
that takes only a `Date` has already thrown away the information needed to choose, because the
choice depends on what was passed to `new Date(...)`, not on the resulting instant.

The type declarations make this invisible. `DatePicker.Props` (lines 104–106) declares all three
properties as `p.Property<string>`, while `init_DatePicker` (lines 124–126) defines them with
`p.Any` under `// TODO (bev) types`. The number encoding that Python always sends is not
representable in the declared type. These lines are not in the diff, but the PR's defect lives in
exactly the union they fail to state.

## Worked code-judo proposal

Take the raw model value instead of a `Date`, so the encoding is still known at the point of
conversion, and make the function a module-level pure function so it can be unit tested without
a view or a browser.

```ts
import {isNumber} from "core/util/types"

// A calendar day reaches the view in one of two encodings:
//  - number: epoch ms from the Python serializer; the day is in the UTC fields
//  - string: written by _on_select() via toDateString(); Date parses it as local time
// Pikaday wants local midnight of that day.
export function to_local_day(value: number | string): Date {
  const d = new Date(value)
  return isNumber(value)
    ? new Date(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate())
    : new Date(d.getFullYear(), d.getMonth(), d.getDate())
}
```

`render()` becomes:

```ts
    const {value, min_date, max_date} = this.model
    this._picker = new Pikaday({
      field: this.input_el,
      defaultDate: to_local_day(value),
      setDefaultDate: true,
      minDate: min_date != null ? to_local_day(min_date) : undefined,
      maxDate: max_date != null ? to_local_day(max_date) : undefined,
      onSelect: (date) => this._on_select(date),
    })
```

and the props state the contract:

```ts
  export type Props = InputWidget.Props & {
    value:    p.Property<number | string>
    min_date: p.Property<number | string | null>
    max_date: p.Property<number | string | null>
  }
```

What this deletes: `_unlocal_date` and its name, the offset arithmetic, the `setTime` mutation,
the `toISOString().substr(0, 10).split('-')` round trip and the three `Number(...)` conversions,
and the comment that describes the mechanics instead of the contract. What it adds: one
`isNumber` branch, placed at the single point where the two encodings meet. `isNumber` already
exists in `bokehjs/src/lib/core/util/types.ts` line 15.

The `prop` column of measurement 1 is this function run under node; it is correct in all nine
zones for both encodings.

Behaviour relative to the head is unchanged for the string encoding (the `_on_select` path that
#9129 is about) and restored to merge-base behaviour for the number encoding.

A more ambitious version would remove the branch as well, by having `_on_select` write the same
encoding Python sends so that only one exists. I have not proposed that as the remedy because it
changes what `cb_obj.value` looks like to `CustomJS` users (the new test pins
`'Mon Sep 16 2019'`), and because the numeric branch of Python's `Date.transform` uses
`datetime.date.fromtimestamp`, which is server-local. It is recorded as a question in the summary.

## Verification status

Verified by execution, offline, outside the clone, under node v24.21.0: all three measurements
above, and the proposed function's results in measurement 1.

Verified by reading only: the Python serialization path (`prepare_value`, `Date.transform`,
`convert_datetime_type`). No Python was executed. The number `1568937600000` used for
`date(2019, 9, 20)` is `Date.UTC(2019, 8, 20)`, which is what the read code computes.

Not verified: anything that needs Pikaday, a browser, or the bokehjs build. Pikaday's source is
not present in the clone (`bokehjs/node_modules` is not installed), so I did not confirm how it
normalises `defaultDate`, `minDate` and `maxDate` internally, and the proposed TypeScript was not
compiled. The claim that the wrong `defaultDate` appears in the input box rests on that being the
same option whose wrong value produced the symptom in #9129.

The clone was not modified; `git status --short` is empty and `HEAD` is
`36549bca3a63d581f7b68d08054a7813c1e6a499`.
