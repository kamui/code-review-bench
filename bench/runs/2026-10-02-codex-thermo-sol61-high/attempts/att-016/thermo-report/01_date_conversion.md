# Date conversion subsystem

## Judgment

F1 is a blocking date-contract regression at `bokehjs/src/lib/models/widgets/date_picker.ts:82–83`. The current helper cannot determine whether its `Date` encodes a UTC calendar date or a local calendar date. The offset adjustment assumes the latter for every input. Preserve the source representation until the DatePicker adapter can interpret it explicitly.

This is one finding with behavioral and structural evidence. The input mutation, ISO round trip, inaccurate pre-existing property types, and missing regression checks support the same boundary diagnosis; they are not separate findings.

## Source evidence

`bokeh/models/widgets/inputs.py:253–263` defines `value`, `min_date`, and `max_date` as Python `Date` properties. The `Date` property accepts Python dates and datetimes, and parses strings to date objects in `bokeh/core/property/datetime.py:58–74`. Python datetimes inherit from dates, so the datetimes used by the new integration tests are accepted.

`bokeh/core/json_encoder.py:184–186` delegates date serialization to `convert_datetime_type`. In `bokeh/util/serialization.py:177–184`, a date or naive midnight datetime is serialized as milliseconds from the naive epoch, i.e. UTC-midnight milliseconds for the intended day. The existing test at `bokeh/util/tests/test_serialization.py:117–118` explicitly expects identical midnight milliseconds for a date and a midnight datetime.

`DatePickerView.render()` constructs `new Date(this.model.value)` at line 68 and does the same for non-null limits at lines 70–71. All three go through `_unlocal_date`. Conversely, `_on_select` stores `date.toDateString()` at line 95. That string parses as a local date in the permitted Node runtime. The JavaScript default at line 124 also uses `toDateString()`.

Before this PR, the helper took the UTC date portion of the parsed `Date`, split it, and constructed a local midnight. That correctly preserved Python UTC timestamps but shifted local-midnight selections backward in eastern time zones. The PR adds `getTimezoneOffset() * 60000` and mutates the argument by subtracting that offset. The resulting UTC fields now equal the original local fields. That repairs local date strings, but for a UTC-midnight timestamp in Los Angeles the local fields already belong to the preceding day.

For September 20, 2019 in Los Angeles, UTC midnight is September 19 at 17:00 local time. The offset is +420 minutes. Subtracting seven hours produces September 19 at 17:00 UTC; the ISO prefix is then September 19. The local date reconstructed from that prefix is the wrong day. September 1 and September 20 limits follow the identical path and become August 31 and September 19.

The passed `Date` objects are freshly created at every current call site. Although removing mutation improves the boundary, no additional shared-object corruption finding is claimed.

## Measurements and commands

The committed diff is two files, +104/−2. `date_picker.ts` grows from 125 to 129 lines and adds no conditional branches. The four-line net growth is small; the issue is the extra implicit conversion assumption, not file size.

The following read-only commands established the source and measurements:

```sh
git diff main...review-head
git diff --numstat main...review-head
nl -ba bokehjs/src/lib/models/widgets/date_picker.ts
git show main:bokehjs/src/lib/models/widgets/date_picker.ts
git show main:bokehjs/src/lib/models/widgets/date_picker.ts | wc -l
nl -ba bokeh/util/serialization.py
nl -ba bokeh/core/property/datetime.py
nl -ba bokeh/core/json_encoder.py
rg -n 'getUTCFullYear|getUTCMonth|getUTCDate|toDateString|new Date\(' bokehjs/src/lib/core/util bokehjs/src/lib/core/properties.ts bokehjs/src/lib/models/widgets/input_widget.ts
```

The last scoped search found no date-only converter to reuse in those utilities. The neighboring date sliders use `timezone` for formatting numeric values, not for bridging date-only model values to a local calendar widget. The Python serializer is already canonical for transport and should retain its existing behavior.

## Focused execution

The scratch harness reads the head source, reads the base with `git show`, extracts each actual `_unlocal_date` body, and executes it unchanged as JavaScript. It runs outside the clone, using only Node and read-only source access:

```sh
node /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-016/clone-work/date-boundary-check.cjs
```

The observed runtime is Node `v24.21.0`. The matrix contains six dates: September 20, September 1, September 16, January 1, March 31, and October 27 of 2019. For each zone it tests UTC-midnight numeric milliseconds, `toDateString()` selections, and ISO date-only strings. It compares the returned local year/month/day to the intended calendar date.

| Time zone | Cases | Base failures | Head failures | Proposal failures |
| --- | ---: | ---: | ---: | ---: |
| UTC | 18 | 0 | 0 | 0 |
| Europe/Paris | 18 | 6 | 0 | 0 |
| Europe/London | 18 | 4 | 0 | 0 |
| America/Los_Angeles | 18 | 0 | 12 | 0 |
| America/New_York | 18 | 0 | 12 | 0 |
| Asia/Kolkata | 18 | 6 | 0 | 0 |
| Pacific/Kiritimati | 18 | 6 | 0 | 0 |

London's base failures occur on the four tested dates with positive daylight-saving offsets. Los Angeles and New York fail all six numeric and all six ISO date-only cases at the head. The head's local selection strings remain correct. This isolates representation provenance as the cause rather than a specific month, boundary, or DST date.

Raw results are in `../date-boundary-results.json`; the harness is `../date-boundary-check.cjs`. Both are scratch review artifacts, not project tests.

## Worked code-judo proposal

Move the parsing decision to the existing widget boundary, before `new Date` destroys information about the incoming representation. Use numeric UTC fields for Python-serialized dates and local fields for the legacy strings emitted by `_on_select`. Handle an ISO date-only string explicitly so that a direct BokehJS date-only value also remains a calendar date. This uses one adapter for all three properties and deletes the offset/mutation/ISO/split pipeline for timestamps and selection strings.

The following sketch is the proposal exercised in the harness, expressed as TypeScript. It is not an applied patch or a claim that project type checking has run:

```ts
type PickerDateValue = number | string

function date_for_picker(value: PickerDateValue): Date {
  if (typeof value == "string" && /^\d{4}-\d{2}-\d{2}$/.test(value)) {
    const [year, month, day] = value.split("-").map(Number)
    return new Date(year, month - 1, day)
  }
  const date = new Date(value)
  return typeof value == "number"
    ? new Date(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate())
    : new Date(date.getFullYear(), date.getMonth(), date.getDate())
}
```

Then `render()` calls `date_for_picker(this.model.value)` directly and applies the same function to each non-null limit. Update the existing string-only property annotations to describe the numeric and string inputs, with nullability retained for optional limits. Keep the conversion local to DatePicker unless another consumer actually needs the same calendar-date contract.

The regular-expression case recognizes only the explicit ISO date-only representation. It is a format discriminator at the boundary, not an attempt to validate every date syntax. The proposal covers the actual Python numeric transport, browser-produced selection strings, the JavaScript default, and ISO date-only strings. Arbitrary timezone-bearing datetime strings, invalid dates, and years outside the existing constructor behavior require a separately specified contract before expanding this adapter. No new fallback is proposed for them.

Leave `_on_select`'s `toDateString()` assignment in place. The new integration test expressly observes that value, and changing it to a numeric timestamp or ISO string would alter observable JavaScript callback behavior. A global serializer change or a new moment dependency is unnecessary to solve this boundary.

The proposal introduces explicit representation discrimination, but removes the hidden interpretation of an already-parsed `Date`. Its purpose is fewer conversion concepts and a pure, testable invariant, not minimum line count. It passes the 126-case focused matrix without offset calculations or argument mutation.

## Verification status and next action

Confirmed: the exact head helper returns the wrong local date for the Python numeric transport in both tested western time zones. Confirmed: the same helper is called for both limits. Confirmed: the proposal preserves intended calendar fields in the focused matrix.

Not executed: project compilation, runtime property validation, Pikaday/browser rendering, websocket round trips, or the Selenium suite. The tracked dependency manifest pins a Bokeh Pikaday fork at `6b7258e`; its implementation is not present in the tracked files inspected, and fetching it is outside the execution policy. The erroneous option dates are directly verified; their visible browser consequences still require integration confirmation.

Implement the adapter and its focused tests, then check initial display and boundary buttons in the browser as described in [the integration report](02_integration_coverage.md). Do not accept the current offset patch on the strength of selection-only checks.
