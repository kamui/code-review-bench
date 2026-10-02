# DatePicker conversion boundary

## Verdict and scope

Request changes. The added offset subtraction makes the helper interpret every input as a local calendar date. That assumption fits the selection string but contradicts the Python serialization boundary. This is a concrete behavior regression caused by a representation problem, with a small structural remedy that also deletes the offset arithmetic, mutation, and string round trip.

The reviewed range is `ccb4bcb4c2b841d89b0e88303a97bf4604a5795f..36549bca3a63d581f7b68d08054a7813c1e6a499`, inspected with `git diff main...review-head`. The checkout was at the pinned head. No repository guidance was loaded as instructions, no external discussion was fetched, and no independent reviewer was used.

## Finding: Preserve the date’s representation at the widget boundary (P1)

In `bokehjs/src/lib/models/widgets/date_picker.ts:82–83`, subtracting `getTimezoneOffset()` from every input changes UTC-encoded Python dates into the browser’s local calendar dates. Python supplies dates as epoch milliseconds, while `_on_select()` supplies a local `toDateString()` string; `render()` erases that distinction by constructing a `Date` before calling the helper. Under `TZ=America/Los_Angeles`, the head helper converts Python’s September 20, 2019 value to September 19, its September 1 minimum to August 31, and its September 30 maximum to September 29; the base helper preserves all three. Retain the raw representation until the widget boundary can choose UTC calendar fields for serialized timestamps and local calendar fields for selection strings, then construct a fresh local date directly. This removes the unconditional offset adjustment, input mutation, ISO formatting, splitting, and reparsing without sacrificing the UTC+ selection fix.

## Source evidence

The helper is used for all three Pikaday inputs in `date_picker.ts:66–73`: `defaultDate`, `minDate`, and `maxDate`. Each caller wraps its model property in `new Date(...)`, so an epoch number and a human-readable local date string become indistinguishable before the helper runs.

The Python model defines `value`, `min_date`, and `max_date` using the `Date` property in `bokeh/models/widgets/inputs.py:253–263`. `bokeh/core/property/datetime.py:58–69` converts accepted strings to Python dates. `bokeh/core/json_encoder.py:184–186` sends recognized dates through `convert_datetime_type()`. That canonical serializer uses a UTC epoch (`bokeh/util/serialization.py:83`) and turns dates into epoch milliseconds (`:177–184`). A midnight Python date consequently arrives as the UTC instant for that calendar date, regardless of the browser’s time zone.

The selection path is different. `date_picker.ts:90–96` writes `date.toDateString()` into the model. Parsing `Mon Sep 16 2019` produces a local midnight date. The base helper extracts its UTC day, which is September 15 in Paris. The added offset adjustment corrects that selection path but applies the same interpretation to the UTC timestamps used for initialization and bounds.

For the Los Angeles initial value, `new Date(Date.UTC(2019, 8, 20))` represents September 19 at 17:00 local time. Its offset is 420 minutes. Subtracting seven hours makes the timestamp `2019-09-19T17:00:00.000Z`; the following ISO slice extracts September 19. The returned local `Date` therefore has the wrong calendar day. The two bounds go through the identical calculation. Passing these changed dates to Pikaday shifts the permitted interval; that consequence follows from the arguments, although no browser was available to execute the calendar UI.

The new `setTime()` also changes the caller’s `Date` object. Current call sites allocate temporary dates, so there is no separately demonstrated aliasing bug. Mutation is part of the unnecessarily indirect conversion and should disappear with the remedy, rather than become an additional finding.

## Measurements and verification

The source file grows from 125 to 129 lines. `tests/integration/widgets/test_datepicker.py` is 98 lines. Neither crosses the skill’s 1000-line threshold. No conditional branches, async orchestration, or shared serializer changes are added by this PR. The defect is concentrated in the conversion boundary, not file sprawl or scattered feature checks.

Read-only inspection commands included `git diff main...review-head`, `git show main:bokehjs/src/lib/models/widgets/date_picker.ts`, line-numbered reads of the changed file and serialization sources, `git diff --numstat main...review-head`, and `wc -l` on the changed paths. A targeted search of `bokehjs/src/lib/core/util` for UTC calendar accessors, `toDateString`, and timezone-offset handling found no existing matching calendar conversion helper. The Python serializer is the canonical wire conversion and should remain untouched.

The offline harness is `../date-probe.cjs`, outside the clone. It extracts the head helper body from the actual TypeScript source and evaluates that JavaScript body. The base helper is a verbatim transcription of the body shown by `git show main:...`. Each helper receives a fresh `Date`. Results are retained in `../date-probe-results.jsonl`.

The harness was executed with separate Node processes under `UTC`, `America/Los_Angeles`, `Europe/Paris`, `Asia/Kolkata`, and `Pacific/Kiritimati`. Each process checks six primary cases and twelve cases spanning DST transition dates, a year boundary, and leap day. The installed runtime reports `v24.21.0`; the packet’s freeze description mentions `v24.19.0`. No runtime was installed or fetched.

| Input and expected calendar day | Base, Los Angeles | Head, Los Angeles | Head, Paris |
| --- | --- | --- | --- |
| Python value: 2019-09-20 | 2019-09-20 | 2019-09-19 | 2019-09-20 |
| Python minimum: 2019-09-01 | 2019-09-01 | 2019-08-31 | 2019-09-01 |
| Python maximum: 2019-09-30 | 2019-09-30 | 2019-09-29 | 2019-09-30 |
| Local selection: 2019-09-16 | 2019-09-16 | 2019-09-16 | 2019-09-16 |
| ISO date string: 2019-09-20 | 2019-09-20 | 2019-09-19 | 2019-09-20 |

In Paris, the base selection result is September 15, demonstrating that the proposed remediation must preserve the PR’s intended fix. The proposed boundary conversion passes all 90 calendar assertions across the five zones. The head fails six of the twelve extra cases in Los Angeles, all on numeric dates. Its input mutation is minus seven hours in Los Angeles and plus two hours in Paris.

These are verified JavaScript conversion results. Python serialization and Pikaday call-site evidence were verified by source inspection. TypeScript compilation, the Python integration suite, actual browser rendering, and the complete server round trip were not executed because the execution policy makes them unavailable.

## Worked code-judo proposal

Keep the conversion in the DatePicker adapter. Preserve raw input until its representation can be interpreted. A scoped implementation sketch is:

```ts
type DateValue = number | string

function picker_date(value: DateValue): Date {
  const date = new Date(value)
  const utc = typeof value == "number" || /^\d{4}-\d{2}-\d{2}(?:T|$)/.test(value)
  return utc
    ? new Date(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate())
    : new Date(date.getFullYear(), date.getMonth(), date.getDate())
}
```

Call this with the raw `model.value`, `model.min_date`, and `model.max_date`, preserving the existing guards around absent bounds. Reflect numeric wire values in the widget’s property types, instead of declaring every runtime value to be a string. The existing `p.Any` declarations and string-only TypeScript declarations predate the PR; they explain the representation ambiguity and are not separate regressions.

This sketch recognizes the demonstrated numeric wire dates, local `toDateString()` output, and ISO inputs. The ISO classification retains UTC calendar interpretation for the existing bare ISO-date and ISO timestamp path. The local-string path retains the existing JavaScript date parser. It is not a proposed general-purpose parser for every possible string format. Before applying it, document supported JavaScript inputs and add compatibility cases for any other formats the widget promises to accept. The arithmetic of this sketch was executed in the probe; its TypeScript integration was not compiled.

The branch belongs at this boundary because the inputs have different meanings. After that distinction is made, the algorithm is simply “extract the appropriate calendar fields and create a local date.” There is no reason to adjust an instant by an offset, serialize it, split a string, and parse three numbers. The existing selection callback can continue returning its current string, avoiding a gratuitous callback API change.

A broader alternative is to normalize all widget dates to a canonical date-only representation and have selection emit that representation. That could remove the compatibility branch later, but changes observable JavaScript callback values and requires an explicit compatibility decision. It is not necessary to fix this PR and should not be bundled into the mandatory remediation.

## Actionable remediation

Replace the helper and its three call sites together so that the representation distinction is retained. Keep global serialization unchanged. Test numeric initialization and both bounds as well as local selection strings in positive, negative, and zero-offset zones. Add browser assertions for the initial display and permitted boundary days when the normal build and browser are available. The separate coverage report describes how to make those checks deterministic and how to retain the existing integration harness.
