# 01 — bokehjs `DatePickerView` date normalisation

Scope: `bokehjs/src/lib/models/widgets/date_picker.ts` (+6 / −2), reviewed over `main...review-head`
(`ccb4bcb4c..36549bca3`). The file is 129 lines after the change, so file size is not a concern.

## Context: what flows into `_unlocal_date`

`_unlocal_date` is called three times in `render()` (`date_picker.ts:68`, `:70`, `:71`), always on
`new Date(<model property>)` for `value`, `min_date` and `max_date`. Those model properties hold
values in two different shapes, depending on who wrote them last:

1. **Python-supplied values.** The Python `DatePicker.value` / `min_date` / `max_date` are `Date`
   properties (`bokeh/models/widgets/inputs.py:253-263`). `Date.transform`
   (`bokeh/core/property/datetime.py:58-69`) coerces strings and numbers into `datetime.date`, and
   `convert_datetime_type` (`bokeh/util/serialization.py:177-184`) serialises a `date`/`datetime` as
   milliseconds since the epoch measured **at UTC midnight**. The PR's own round-trip test relies on
   this shape (`tests/integration/widgets/test_datepicker.py:94-97` decodes with `utcfromtimestamp`).
   So every initial value, and every `min_date` / `max_date`, reaches the browser as a UTC-midnight
   timestamp.
2. **Browser-selected values.** `_on_select` (`date_picker.ts:90-97`) writes
   `date.toDateString()`, e.g. `"Mon Sep 16 2019"`. `new Date("Mon Sep 16 2019")` parses as
   **local midnight**.

The original `_unlocal_date` read the calendar date from the UTC components (`toISOString`). That is
right for shape 1 and wrong for shape 2 in any zone east of UTC, where local midnight is still the
previous day in UTC. That is issue #9129: after a pick, the widget re-renders (`connect_signals`
re-renders on any model change, `:54`) and shows the previous day.

## Finding 1.1 — The fix moves the off-by-one from UTC+ zones to UTC− zones instead of removing it

**Severity: blocker (correctness regression plus a structural root cause left unaddressed).
Status: verified by execution (node v24, `TZ` varied).**

The new code at `date_picker.ts:82-83` shifts the instant by `-getTimezoneOffset()` before calling
`toISOString()`. That turns the function into "read the calendar date from the **local**
components". This fixes shape 2 everywhere, but it breaks shape 1 in every zone west of UTC. A
UTC-midnight timestamp for 2019-09-20 is 17:00 on 2019-09-19 in Los Angeles, so the widget now
shows the previous day for the Python-supplied initial `value`, `min_date` and `max_date` across the
Americas and other UTC− zones. Before this PR, all of those were correct.

Reproduction (`clone-work/scratch/unlocal.js`, which copies the pre-PR body, the PR body and a
one-line equivalent verbatim):

```
$ for tz in UTC Europe/Paris Europe/London Asia/Tokyo America/Los_Angeles America/New_York Pacific/Kiritimati Pacific/Pago_Pago; do TZ=$tz node unlocal.js; done
Europe/Paris         python ms (UTC midnight 2019-09-20)    old: Fri Sep 20 2019 | new: Fri Sep 20 2019 | ...
Europe/Paris         toDateString "Fri Sep 20 2019"         old: Thu Sep 19 2019 | new: Fri Sep 20 2019 | ...
America/Los_Angeles  python ms (UTC midnight 2019-09-20)    old: Fri Sep 20 2019 | new: Thu Sep 19 2019 | ...
America/Los_Angeles  toDateString "Fri Sep 20 2019"         old: Fri Sep 20 2019 | new: Fri Sep 20 2019 | ...
America/Los_Angeles  ISO date "2019-09-20"                  old: Fri Sep 20 2019 | new: Thu Sep 19 2019 | ...
America/New_York     python ms (UTC midnight 2019-09-20)    old: Fri Sep 20 2019 | new: Thu Sep 19 2019 | ...
Pacific/Pago_Pago    python ms (UTC midnight 2019-09-20)    old: Fri Sep 20 2019 | new: Thu Sep 19 2019 | ...
```

(Full output: UTC is correct for both versions. Every UTC+ zone is fixed for the `toDateString`
shape. Every UTC− zone regresses for the timestamp shape and the ISO-string shape.)

Concretely, with the issue's own reproducer, `DatePicker(value=datetime.date.today())`, a user in
New York now sees yesterday's date as soon as the page loads. `min_date` / `max_date` also slip by
one day, so the last allowed day becomes unselectable. The PR comment says the approach is
"agnostic to their local systems's timezone" (`:81`). It is not. It swaps which timezone half of
the world gets the wrong answer.

The root cause is structural. The model's date properties have **no single canonical
representation**. Python writes UTC-anchored timestamps and the view writes local-anchored
`toDateString()` strings. No single "timestamp → calendar date" rule can be right for both
everywhere. Any fix that only edits the reader, as this PR does, can only choose which writer to
break.

### Worked code-judo proposal

Make the writer emit the same representation Python already emits, and keep one UTC-anchored
reader. `new Date("YYYY-MM-DD")` is specified to parse as UTC midnight, and Python's `Date.transform`
parses `"YYYY-MM-DD"` with `dateutil` into the exact `date`. A date-only ISO string built from
**local** components is therefore the canonical format on both sides. It is also not the
`toISOString()` bug from #7048, because it never passes through UTC conversion.

```ts
// date_picker.ts
function to_iso_date(date: Date): string {          // pikaday hands back local midnight
  const pad = (n: number) => `${n < 10 ? "0" : ""}${n}`
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

function to_local_date(value: string | number): Date { // model values are UTC-midnight anchored
  const d = new Date(value)
  return new Date(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate())
}

// render():
//   defaultDate: to_local_date(this.model.value),
//   minDate: this.model.min_date != null ? to_local_date(this.model.min_date) : undefined,
//   maxDate: this.model.max_date != null ? to_local_date(this.model.max_date) : undefined,

_on_select(date: Date): void {
  this.model.value = to_iso_date(date)
  this.change_input()
}
```

Verified with `clone-work/scratch/judo.js` in UTC, Europe/Paris, Asia/Tokyo, Pacific/Kiritimati
(+14), America/Los_Angeles and Pacific/Pago_Pago (−11). Both the Python timestamp for 2019-09-20
and a picked 2019-09-16 round-trip to the correct calendar day in every zone.

This deletes the `setTime` shift, the `toISOString().substr().split()` string surgery, and the
three-paragraph comment. It also retires the `// XXX: this should be handled by the serializer`
debt in `_on_select`, because the value is now in the format the serializer already speaks. Trade-off: JS
callbacks will see `"2019-09-16"` instead of `"Mon Sep 16 2019"` in `cb_obj.value`, and the
PR's `test_js_on_change_executes` assertion at `test_datepicker.py:66` would change to match. If
that JS-visible format must be frozen, the fallback is to keep `toDateString()` and have
`to_local_date` dispatch once on `typeof value === "number"`: UTC components for numbers, local
components for strings. That is uglier, but it is still correct in every zone, unlike the current
diff.

## Finding 1.2 — The new `_unlocal_date` is a roundabout spelling of "take the local calendar date", and it mutates its argument

**Severity: medium (legibility / hidden side effect). Status: verified by execution.**

After the change, `_unlocal_date` (`date_picker.ts:78-88`) does four things. It shifts the
instant by the local offset, formats it as an ISO string, slices and splits that string, and
reassembles a local `Date`. Across every zone and input shape tested above, this produces the same
result as

```ts
new Date(date.getFullYear(), date.getMonth(), date.getDate())
```

(`judo` column in `unlocal.js`: identical to `new` in all 24 rows). The offset arithmetic plus ISO
round-trip exists only to cancel itself out. A reader has to simulate timezone maths to discover
that the function just reads local components.

The function also mutates its parameter through `date.setTime(...)` (`:83`). The script confirms
the caller's `Date` is changed in every non-UTC zone. Today every call site passes a freshly
constructed `new Date(...)`, so nothing observable breaks. But a method whose name suggests a pure
conversion now has a side effect on its input. That is a trap for the next caller, and it is exactly
the kind of incidental control flow this review should push back on.

The comment block (`:79-81`) describes the mechanics ("multiply to get the offset in ms") rather
than the invariant. It also asserts timezone-agnosticism that Finding 1.1 shows is false.

Remedy: adopt the proposal in 1.1, which removes this function entirely. If the PR is kept as a
minimal patch, at least replace the body with the one-liner above, make it a pure module-level
function, and state the actual contract in one line, e.g. "model value → local midnight of the
same calendar day".

## Non-findings checked

- File size: 129 lines, far from the 1k threshold.
- Call sites: all three are in `render()` and all pass fresh `Date` objects, so the mutation in 1.2
  is latent, not live.
- DST: `getTimezoneOffset()` is read from the date itself, so DST transitions do not add a separate
  error beyond Finding 1.1.
