# DatePicker representation boundary

## Judgment

The blocking finding is the UTC/local boundary regression at `bokehjs/src/lib/models/widgets/date_picker.ts:82–83`. The added offset operations do not make dates timezone agnostic. They change the helper from extracting UTC calendar fields to extracting local calendar fields. That fixes one input provenance and breaks another. The remedy should resolve provenance at the widget boundary and delete the conversion machinery, rather than add another offset exception.

## Source evidence

`DatePickerView.render`, at lines 66–73, constructs Pikaday with three independently supplied dates. It calls `_unlocal_date(new Date(this.model.value))` for the default and the same conversion for each non-null bound. Wrapping the model values in `new Date` before entering the helper erases the distinction between a UTC-encoded timestamp and a string parsed as local midnight.

`bokeh/models/widgets/inputs.py:253–263` declares all three attributes as Python `Date` properties. `bokeh/core/property/datetime.py:58–69` transforms supplied strings into Python dates and otherwise retains date/datetime objects. `bokeh/core/json_encoder.py:184–186` delegates those objects to `convert_datetime_type`. In `bokeh/util/serialization.py:177–184`, both Python dates and naive datetimes become milliseconds since a naive UTC epoch; a date represents midnight on its given day. Thus a normal Python `DatePicker(value=date(2019, 9, 20))` supplies the timestamp `1568937600000` to the JavaScript view.

Browser selection has different provenance. `date_picker.ts:90–96` sets the model value to `date.toDateString()`, deliberately omitting a timezone. For example, `Mon Sep 16 2019` parses at local midnight. The base helper's `toISOString().substr(0, 10)` retrieves the previous UTC day for such strings in UTC+ zones. The new subtraction fixes that selection case, but its unconditional use on Python's UTC-midnight timestamps interprets their local day instead of the encoded date.

The three added comment lines describe offset mechanics without specifying these two representations. The existing TypeScript `p.Property<string>` declarations at lines 103–106 do not capture the observed numeric wire values; that mismatch predates this patch. It is relevant to a replacement boundary contract, not a separate newly introduced type finding.

## Measurements and commands

The review used these read-only commands in the checkout:

```sh
git rev-parse HEAD main review-head
git diff --stat main...review-head
git diff main...review-head
nl -ba bokehjs/src/lib/models/widgets/date_picker.ts
nl -ba bokeh/core/property/datetime.py
nl -ba bokeh/models/widgets/inputs.py
nl -ba bokeh/core/json_encoder.py
nl -ba bokeh/util/serialization.py
rg -n 'getFullYear|getUTCFullYear|toISOString\(\).*substr|60000|getTimezoneOffset' bokehjs/src/lib bokehjs/test
wc -l bokehjs/src/lib/models/widgets/date_picker.ts tests/integration/widgets/test_datepicker.py
git show main:bokehjs/src/lib/models/widgets/date_picker.ts | wc -l
git diff --check main...review-head
```

No existing date-only picker adapter appeared in the focused search. UTC field access elsewhere in date tickers operates on axis timestamps and does not provide a reusable picker-boundary abstraction. The fix belongs in DatePicker; broad changes to general serialization would create unnecessary coupling.

The base helper source was copied to `base-date-picker.ts` outside the checkout with `git show main:bokehjs/src/lib/models/widgets/date_picker.ts`. The following permitted offline command ran from the work directory:

```sh
node /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-030/clone-work/thermo-report/check-dates.cjs
```

The harness extracts each helper body by its TypeScript signature and executes it as JavaScript. Each timezone runs in a separate Node process with `TZ` set before date construction. No Bokeh bundle, DOM, browser, dependencies, or network are involved.

| Input | Wanted date | Base in Los Angeles | Head in Los Angeles | Head in Paris |
| --- | --- | --- | --- | --- |
| Python timestamp value | 2019-09-20 | 2019-09-20 | 2019-09-19 | 2019-09-20 |
| Python timestamp minimum | 2019-09-01 | 2019-09-01 | 2019-08-31 | 2019-09-01 |
| Python timestamp maximum | 2019-09-30 | 2019-09-30 | 2019-09-29 | 2019-09-30 |
| ISO date-only string | 2019-09-20 | 2019-09-20 | 2019-09-19 | 2019-09-20 |
| Local selection string | 2019-09-16 | 2019-09-16 | 2019-09-16 | 2019-09-16 |

New York gives the same wrong calendar dates as Los Angeles. At Paris the base selection case returns September 15, confirming that the patch addresses the originating defect. Kolkata and Kiritimati also demonstrate that selection improvement; UTC makes all five base and head cases indistinguishable.

There are eight incorrect head results across 30 representative cases. Offset mutation is observable in the harness: the caller's Date moves by minus seven hours in Los Angeles and plus two hours in Paris on these September samples. The real callers currently supply fresh Date instances, so this review does not claim a separate caller-owned-state corruption bug. The mutation is unnecessary work and obscures the helper's contract.

## Worked code-judo proposal

The new arithmetic has a direct equivalent for valid modern dates: it arranges for the ISO fields to equal the original local fields, splits that string, parses it back to numbers, and reconstructs local midnight. Replace that entire chain with component access:

```ts
return new Date(date.getFullYear(), date.getMonth(), date.getDate())
```

The harness checks this behavior-preserving simplification against head for eight selected dates, 24 hours on each date, and six zones: 1,152 matches. Merely making this substitution is insufficient, because it preserves the head regression on UTC timestamps. It identifies the machinery that can be deleted once the boundary is corrected.

A scoped replacement keeps the raw value until it decides which calendar fields represent the requested date:

```ts
type PickerDateValue = number | string

function to_picker_date(value: PickerDateValue): Date {
  const date = new Date(value)
  const utc = typeof value === "number" || /^\d{4}-\d{2}-\d{2}$/.test(value)
  return utc
    ? new Date(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate())
    : new Date(date.getFullYear(), date.getMonth(), date.getDate())
}
```

This is a worked candidate for the evidenced wire representations: numeric Python values, ISO date-only strings, and the widget's own `toDateString()` output. It adds one meaningful decision at the representation boundary while deleting offset math, mutation, formatting, splitting, and numeric reparsing. It should receive `this.model.value` directly; the same adapter receives the raw optional bounds after the existing null checks. A private view helper can own it without creating a new general-purpose module.

The harness exercises this candidate on all three representations for eight dates in six zones, including leap day, year boundaries, and dates near North American and European DST transitions: 144 expected-date assertions pass. The outgoing model assignment remains `date.toDateString()`, so the callback representation and websocket behavior motivating the existing comment remain intact. Python serialization also remains intact. These checks cover dates in 2019–2020; they do not establish a policy for every arbitrary string or historical date JavaScript can parse.

Before adopting the candidate, explicitly document the supported string representations and retain any other documented CustomJS inputs with a corresponding parsing rule and test. Avoid a universal serializer change or a new offset flag passed between render and selection. If preserving the existing helper name is useful, change its input contract to the raw timestamp/string union and update all three render call sites; the current Date-only signature cannot express provenance.

## Verification limits and remediation

The exact helper regression and the proposed adapter's date conversion are verified offline. Pikaday receives the wrongly converted dates through the reviewed call sites; therefore an incorrect initial date and shifted interval are supported by source tracing. The actual browser display, Pikaday's callback ordering, and server event behavior were not executed. No claim is made that the project build or integration suite passed.

Replace the helper and its call-site contract together, then exercise initialization, inclusive bounds, selection rerenders, and server-driven numeric updates with explicit timezone coverage. Preserve the existing outward selection format and keep the conversion inside the widget that owns the UTC-to-local-calendar boundary.
