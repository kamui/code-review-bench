# 01 — DatePicker view (`bokehjs/src/lib/models/widgets/date_picker.ts`)

Covers findings F1 and F2 of `summary.md`.

## The change

```diff
   _unlocal_date(date: Date): Date {
-    // this sucks but the date comes in as a UTC timestamp and pikaday uses Date's local
-    // timezone-converted representation. We want the date to be as given by the user
+    //Get the UTC offset (in minutes) of date (will be based on the timezone of the user's system).
+    //Then multiply to get the offset in ms.
+    //This way it can be used to recreate the user specified date, agnostic to their local systems's timezone.
+    const timeOffsetInMS = date.getTimezoneOffset() * 60000
+    date.setTime(date.getTime() - timeOffsetInMS)
+
     const datestr = date.toISOString().substr(0, 10)
     const tup = datestr.split('-')
     return new Date(Number(tup[0]), Number(tup[1])-1, Number(tup[2]))
```

The file is 129 lines at the head; size is not a concern.

## What actually arrives at `_unlocal_date`

The three call sites are `date_picker.ts:68`, `:70` and `:71`. Each wraps the raw model value in `new Date(...)` and hands the result over, so the helper only ever sees a `Date`. The model properties are declared `p.Any` (`date_picker.ts:124-126`, under a `// TODO (bev) types` note) and in practice carry two shapes.

The first shape is a number. On the Python side `DatePicker.value`, `min_date` and `max_date` are `Date` properties (`bokeh/models/widgets/inputs.py:253-263`). `Date.transform` (`bokeh/core/property/datetime.py:58-69`) turns strings and numbers into `datetime.date` and lets `date` and `datetime` objects through; `Property.serialize_value` is the identity (`bokeh/core/property/bases.py:251-255`); the JSON encoder then calls `convert_datetime_type`, which for a `date` returns `(dt.datetime(*obj.timetuple()[:6]) - DT_EPOCH).total_seconds() * 1000` and for a `datetime` returns the naive value's distance from the epoch (`bokeh/util/serialization.py:177-184`). So a Python date arrives as epoch milliseconds at UTC midnight of that calendar date, and a Python datetime arrives as epoch milliseconds whose UTC reading is the naive datetime. In both cases the calendar date the user meant is the UTC date of the instant.

The second shape is a string. `_on_select` writes `date.toDateString()` back to the model (`date_picker.ts:95`), the JavaScript default is `new Date().toDateString()` (`date_picker.ts:124`), and `bokehjs/test/integration.ts:584` constructs the model the same way. `new Date("Mon Sep 16 2019")` is local midnight. Here the calendar date the user meant is the local date of the instant. Because the view re-renders on every model change (`date_picker.ts:54`), this shape reaches `_unlocal_date` immediately after every pick.

A function that receives only the instant has no way to know whether to read it in UTC or in local time. The merge-base read everything in UTC; the head reads everything in local time (see F2 below). Each is wrong for the other shape wherever the two readings disagree, and they disagree on opposite sides of UTC.

## Measurements

Both versions were transcribed verbatim into a scratch script outside the clone and run under `TZ`. `want` is the calendar date the user supplied.

Command:

```
for tz in UTC Europe/Paris America/Los_Angeles America/New_York Asia/Kolkata; do TZ=$tz node unlocal.js table; done
```

| Input reaching the view | Zone | Merge-base | Head | Proposal A |
| --- | --- | --- | --- | --- |
| Python `date(2019, 9, 20)` → ms | UTC | Fri Sep 20 | Fri Sep 20 | Fri Sep 20 |
| | Europe/Paris | Fri Sep 20 | Fri Sep 20 | Fri Sep 20 |
| | Asia/Kolkata | Fri Sep 20 | Fri Sep 20 | Fri Sep 20 |
| | America/Los_Angeles | Fri Sep 20 | **Thu Sep 19** | Fri Sep 20 |
| | America/New_York | Fri Sep 20 | **Thu Sep 19** | Fri Sep 20 |
| Python `datetime(2019, 9, 1)` as `min_date` → ms | America/Los_Angeles | Sun Sep 01 | **Sat Aug 31** | Sun Sep 01 |
| | America/New_York | Sun Sep 01 | **Sat Aug 31** | Sun Sep 01 |
| Python `datetime(2019, 9, 20, 3, 0)` → ms | America/Los_Angeles | Fri Sep 20 | **Thu Sep 19** | Fri Sep 20 |
| | America/New_York | Fri Sep 20 | **Thu Sep 19** | Fri Sep 20 |
| Python `datetime(2019, 9, 20, 23, 0)` → ms | Europe/Paris | Fri Sep 20 | **Sat Sep 21** | Fri Sep 20 |
| | Asia/Kolkata | Fri Sep 20 | **Sat Sep 21** | Fri Sep 20 |
| `_on_select` string `"Mon Sep 16 2019"` | Europe/Paris | **Sun Sep 15** | Mon Sep 16 | Mon Sep 16 |
| | Asia/Kolkata | **Sun Sep 15** | Mon Sep 16 | Mon Sep 16 |
| | America/Los_Angeles | Mon Sep 16 | Mon Sep 16 | Mon Sep 16 |
| | America/New_York | Mon Sep 16 | Mon Sep 16 | Mon Sep 16 |

Rows not listed (every input under UTC, and the remaining zone/input combinations) were correct in all three columns. Bold cells are wrong.

Reading the table: the merge-base has exactly one failing family, the string written by a pick, east of UTC. That is issue #9129. The head fixes that family and introduces two new ones: every Python-supplied value west of UTC, and Python datetimes late in the UTC day east of UTC. The first of these is worse than the bug being fixed, because it is the first paint of the widget, it applies to `min_date` and `max_date` as well (so the selectable range is shifted a day), and it comes back every time Python pushes a new value.

### F2: the head helper is local-midnight truncation

Command:

```
for tz in UTC Europe/Paris America/Los_Angeles America/New_York Asia/Kolkata America/Sao_Paulo; do TZ=$tz node unlocal.js equiv; done
```

Output:

```
UTC                  head_unlocal(d) vs local-midnight truncation of d: 28411 instants sampled, 0 differ
Europe/Paris         head_unlocal(d) vs local-midnight truncation of d: 28411 instants sampled, 0 differ
America/Los_Angeles  head_unlocal(d) vs local-midnight truncation of d: 28411 instants sampled, 0 differ
America/New_York     head_unlocal(d) vs local-midnight truncation of d: 28411 instants sampled, 0 differ
Asia/Kolkata         head_unlocal(d) vs local-midnight truncation of d: 28411 instants sampled, 0 differ
America/Sao_Paulo    head_unlocal(d) vs local-midnight truncation of d: 28411 instants sampled, 0 differ
```

The sample steps through 2018 and 2019 in 37-minute increments, which crosses every daylight-saving change in those zones; São Paulo is included because its 2018 switch fell at local midnight. Across all of it the head `_unlocal_date(d)` equals `new Date(d.getFullYear(), d.getMonth(), d.getDate())`.

So the five statements of the head body compute "the local midnight of `d`". Upstream Pikaday applies its own start-of-day truncation to `defaultDate`, `minDate` and `maxDate`. Bokeh pins a fork (`bokehjs/package.json:88`, `https://github.com/bokeh/pikaday.git#6b7258e`) whose source is not in the clone and could not be fetched, so whether the fork keeps that behaviour is **unverified**. If it does, the head is behaviourally `defaultDate: new Date(this.model.value)`, which is the line commit `475828e23` ("update datepicker properly (#9062)") replaced when it introduced `_unlocal_date`:

```diff
-      defaultDate: new Date(this.model.value),
+      defaultDate: this._unlocal_date(new Date(this.model.value)),
```

Independently of Pikaday, the measured table above already shows the visible consequence: the head gives the pre-#9062 answers for Python-supplied values.

### F2: argument mutation

Command and output:

```
for tz in Europe/Paris America/Los_Angeles; do TZ=$tz node unlocal.js mutate; done

Europe/Paris         argument mutated by 2 h
America/Los_Angeles  argument mutated by -7 h
```

`date.setTime(...)` rewrites the caller's object. The three current callers pass a freshly constructed temporary, so nothing observes it today, but a method that returns a new `Date` and also silently shifts its input is a trap for the next caller.

## Verification status

| Claim | Status |
| --- | --- |
| Head renders Python-supplied dates a day early west of UTC | Verified by execution (node v24.21.0, `TZ` set), on a verbatim transcription of the method |
| Head renders late-in-day Python datetimes a day late east of UTC | Verified by execution, same method |
| Merge-base was correct for Python-supplied values in all sampled zones | Verified by execution, same method |
| Head helper equals local-midnight truncation | Verified by execution over 28,411 instants × 6 zones |
| Helper mutates its argument | Verified by execution |
| Python sends epoch ms at UTC midnight for `date` | Read from source, not executed |
| The pinned Pikaday fork truncates to start of day | Unverified; fork source absent |
| The widget in a real browser shows the same dates | Not run; no build or browser available |

## Worked proposal A — decode by wire type (behaviour-preserving for both wire shapes)

The judo move is to take the raw model value instead of a `Date`. At that point the wire type still says which reading is correct, and the offset arithmetic, the string round trip and the mutation all disappear.

```ts
import {isNumber} from "core/util/types"

    this._picker = new Pikaday({
      field: this.input_el,
      defaultDate: this._calendar_date(this.model.value),
      setDefaultDate: true,
      minDate: this.model.min_date != null ? this._calendar_date(this.model.min_date) : undefined,
      maxDate: this.model.max_date != null ? this._calendar_date(this.model.max_date) : undefined,
      onSelect: (date) => this._on_select(date),
    })

  // A value is either epoch milliseconds from Python, whose UTC calendar date is the
  // date the user gave, or the local date string written by _on_select. Pikaday wants
  // local midnight of that calendar date.
  protected _calendar_date(value: number | string): Date {
    const date = new Date(value)
    return isNumber(value) ? new Date(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate()) : date
  }
```

`isNumber` already exists at `bokehjs/src/lib/core/util/types.ts:15` and sibling widgets import from that module (`dropdown.ts:7`, `multiselect.ts:2`, `selectbox.ts:2`), so no new helper is introduced. The "Proposal A" column in the table above is this function; it is correct for every input in every sampled zone. Nothing about the wire format changes, so `CustomJS` callbacks and the Python `Date.transform` path see exactly what they see today.

The one branch that remains is honest: it is the two-encoding fact made visible in one place, where the merge-base and the head each hide it behind a formula that only fits one side. Its known limit is a string that is not in the local form the widget writes, for example a `CustomJS` callback assigning an ISO `"2019-09-16"`, which JavaScript parses as UTC midnight. Proposal B removes that limit.

## Worked proposal B — one canonical wire encoding (follow-up; changes what `CustomJS` observes)

If the string form may change (question Q2 in the summary), let `_on_select` write an ISO date built from the local components Pikaday hands it:

```ts
  _on_select(date: Date): void {
    const pad = (n: number) => (n < 10 ? "0" : "") + n
    this.model.value = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
    this.change_input()
  }

  protected _calendar_date(value: number | string): Date {
    const date = new Date(value)
    return new Date(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate())
  }
```

A date-only ISO string parses as UTC midnight, which is the same instant Python sends for that date, so both shapes collapse to one and the branch goes away. The scratch run confirms it in every sampled zone:

```
America/Los_Angeles  propB: write=2019-09-16 same-instant-as-python=true read=Mon Sep 16 2019 read(python ms)=Fri Sep 20 2019
Europe/Paris         propB: write=2019-09-16 same-instant-as-python=true read=Mon Sep 16 2019 read(python ms)=Fri Sep 20 2019
```

Building the string from local components avoids the `toISOString()` off-by-one the existing comment at `date_picker.ts:93` warns about (#7048), and `dateutil.parser.parse("2019-09-16").date()` on the Python side (`bokeh/core/property/datetime.py:67`) needs no timezone at all. The JavaScript default at `date_picker.ts:124` and the fixture at `bokehjs/test/integration.ts:584` would move to the same form, and the properties could then be declared as something narrower than `p.Any`. This is the version that retires the `// XXX: this should be handled by the serializer` note. It is an observable change to `cb_obj.value`, so it belongs in its own PR with a release note.

## Scratch script

`unlocal.js`, written and run outside the clone:

```js
// Transcriptions of DatePickerView._unlocal_date at the merge-base and at the head.
function base_unlocal(date) {
  const datestr = date.toISOString().substr(0, 10)
  const tup = datestr.split('-')
  return new Date(Number(tup[0]), Number(tup[1])-1, Number(tup[2]))
}
function head_unlocal(date) {
  const timeOffsetInMS = date.getTimezoneOffset() * 60000
  date.setTime(date.getTime() - timeOffsetInMS)
  const datestr = date.toISOString().substr(0, 10)
  const tup = datestr.split('-')
  return new Date(Number(tup[0]), Number(tup[1])-1, Number(tup[2]))
}
// Proposal A: decode by wire type, no offset arithmetic.
function proposal_a(value) {
  const d = new Date(value)
  return typeof value == "number" ? new Date(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate()) : d
}
// Proposal B: one canonical wire encoding (ISO date-only string written from local components)
function to_iso(date) {
  const pad = (n) => (n < 10 ? "0" : "") + n
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}
function proposal_b(value) {
  const d = new Date(value)
  return new Date(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate())
}
const show = (d) => d.toDateString()

const cases = [
  ["python date(2019,9,20) -> epoch ms",           Date.UTC(2019, 8, 20),          "Fri Sep 20 2019"],
  ["python datetime(2019,9,1) min_date -> ms",     Date.UTC(2019, 8, 1),           "Sun Sep 01 2019"],
  ["python datetime(2019,9,20,3,0) -> ms",         Date.UTC(2019, 8, 20, 3, 0),    "Fri Sep 20 2019"],
  ["python datetime(2019,9,20,23,0) -> ms",        Date.UTC(2019, 8, 20, 23, 0),   "Fri Sep 20 2019"],
  ["_on_select string 'Mon Sep 16 2019'",          "Mon Sep 16 2019",              "Mon Sep 16 2019"],
]
const mode = process.argv[2]
if (mode == "table") {
  for (const [label, value, want] of cases) {
    const b = show(base_unlocal(new Date(value)))
    const h = show(head_unlocal(new Date(value)))
    const a = show(proposal_a(value))
    const mark = (s) => (s == want ? "ok   " : "WRONG") + " " + s
    console.log(`${process.env.TZ.padEnd(20)} ${label.padEnd(44)} base=${mark(b)}  head=${mark(h)}  propA=${mark(a)}`)
  }
  const picked = new Date(2019, 8, 16)            // what pikaday hands to onSelect: local midnight
  const iso = to_iso(picked)
  console.log(`${process.env.TZ.padEnd(20)} propB: write=${iso} same-instant-as-python=${new Date(iso).getTime() == Date.UTC(2019, 8, 16)} read=${show(proposal_b(iso))} read(python ms)=${show(proposal_b(Date.UTC(2019, 8, 20)))}`)
} else if (mode == "equiv") {
  let n = 0, diff = 0
  for (let t = Date.UTC(2018, 0, 1); t < Date.UTC(2020, 0, 1); t += 37 * 60 * 1000) {
    const h = head_unlocal(new Date(t))
    const d = new Date(t)
    const trunc = new Date(d.getFullYear(), d.getMonth(), d.getDate())
    n++
    if (h.getTime() != trunc.getTime()) diff++
  }
  console.log(`${process.env.TZ.padEnd(20)} head_unlocal(d) vs local-midnight truncation of d: ${n} instants sampled, ${diff} differ`)
} else if (mode == "mutate") {
  const d = new Date(Date.UTC(2019, 8, 20))
  const before = d.getTime()
  head_unlocal(d)
  console.log(`${process.env.TZ.padEnd(20)} argument mutated by ${(d.getTime() - before) / 3600000} h`)
}
```
