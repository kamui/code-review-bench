# Date conversion boundary

## Judgment and scope

F1 is a P1 regression at `bokehjs/src/lib/models/widgets/date_picker.ts:82–83`. The six added lines strengthen an incorrect invariant: every incoming `Date` is assumed to represent a locally parsed calendar date. That holds for browser selections but not for Python's serialized dates. The remedy is to preserve the representation at the widget boundary and construct a local calendar date from the appropriate fields.

This detail report was produced by the primary reviewer. No child review or alternate-model review was performed. Only repository source, the supplied packet, and the selected frozen skill informed the review. The packet's prior review comments were not used as correctness evidence.

## Source evidence

`render()` at lines 66–73 constructs Pikaday with three calls to the helper: `defaultDate` from `model.value`, `minDate` from `model.min_date`, and `maxDate` from `model.max_date`. Each call wraps the raw model value in `new Date(...)` first. The original representation is therefore unavailable inside `_unlocal_date`.

`_on_select()` at lines 90–96 stores `date.toDateString()`. For a local September 16 selection this produces `Mon Sep 16 2019`. Reconstructing that string with `new Date` represents local midnight. In Paris in September, that instant's ISO UTC date is September 15. The base helper reads the UTC ISO fields and therefore reproduces the originating issue. The head's added shift corrects this specific path.

Python's properties at `bokeh/models/widgets/inputs.py:248–264` are `Date` values for the initial value and both limits. `Date.transform()` at `bokeh/core/property/datetime.py:58–69` preserves supplied `datetime.date` and `datetime.datetime` objects and parses date strings into date objects. `BokehJSONEncoder.transform_python_types()` at `bokeh/core/json_encoder.py:184–186` delegates these to `convert_datetime_type`.

The serializer at `bokeh/util/serialization.py:177–184` computes milliseconds relative to `DT_EPOCH`, which is defined at line 83 as `datetime.utcfromtimestamp(0)`. A naive September 20, 2019 midnight, including the one in the new tests, is thus sent as `1568937600000`, the instant `2019-09-20T00:00:00.000Z`. In Los Angeles this instant's local calendar fields belong to September 19. The base deliberately reconstructs September 20 from the UTC fields. The new offset shift instead projects September 19 into the ISO fields before reconstructing it.

This changes the arguments passed to Pikaday, even before any user interaction. A minimum of September 1 becomes August 31, while a maximum of September 30 becomes September 29. Pikaday enforcement of those wrong arguments is a source-level consequence, not an executed browser result in this review.

## Measurements and commands

The identity and diff were checked with:

```sh
git rev-parse HEAD main review-head
git diff main...review-head
git diff --numstat main...review-head
git diff --check main...review-head
wc -l bokehjs/src/lib/models/widgets/date_picker.ts tests/integration/widgets/test_datepicker.py
```

The checked-out head and base match the supplied SHAs. The manifest is exactly the two supplied files, with `+6/-2` TypeScript lines and `+98/-0` Python lines. The TypeScript file is 129 lines at head and 125 at base. No whitespace errors were reported. There is no file-size, new-branch, dependency, or orchestration explosion to flag.

The probe is a scratch script outside the checkout. `base()` and `head()` copy the respective helper bodies with their TypeScript annotations removed. Each receives a newly constructed `Date`, matching the production call sites. Calendar results use local year/month/day fields, which are the fields the helper is constructing for Pikaday. Using `toISOString()` to inspect the returned local date would measure the wrong invariant again.

These commands ran from the work directory:

```sh
node --version
TZ=UTC node date-picker-probe.cjs
TZ=America/Los_Angeles node date-picker-probe.cjs
TZ=Europe/Paris node date-picker-probe.cjs
TZ=Asia/Kolkata node date-picker-probe.cjs
TZ=Pacific/Honolulu node date-picker-probe.cjs
```

The installed runtime reports `v24.21.0`. Every process exited successfully. The captured output is `../date-picker-probe-results.json`.

Representative executed results:

| Process timezone | Raw input | Expected local calendar day | Base | Head | Boundary proposal |
| --- | --- | --- | --- | --- | --- |
| UTC | `1568937600000` | 2019-09-20 | 2019-09-20 | 2019-09-20 | 2019-09-20 |
| America/Los_Angeles | `1568937600000` | 2019-09-20 | 2019-09-20 | 2019-09-19 | 2019-09-20 |
| America/Los_Angeles | `1567296000000` | 2019-09-01 | 2019-09-01 | 2019-08-31 | 2019-09-01 |
| America/Los_Angeles | `1569801600000` | 2019-09-30 | 2019-09-30 | 2019-09-29 | 2019-09-30 |
| Europe/Paris | `Mon Sep 16 2019` | 2019-09-16 | 2019-09-15 | 2019-09-16 | 2019-09-16 |
| Asia/Kolkata | `Mon Sep 16 2019` | 2019-09-16 | 2019-09-15 | 2019-09-16 | 2019-09-16 |
| Pacific/Honolulu | `1568937600000` | 2019-09-20 | 2019-09-20 | 2019-09-19 | 2019-09-20 |

Winter cases reproduce the regression with Los Angeles's eight-hour offset, so it is not a summer-only issue. A date-only ISO string also regresses in UTC− zones because JavaScript interprets that particular string form as UTC. All 35 expectations asserted for the boundary proposal pass.

A separate probe feeds the converter first a selection string and then the UTC milliseconds for the same calendar date. In Los Angeles the head returns September 16 and September 15 respectively. This demonstrates sensitivity to representation. It does not claim that every websocket exchange echoes a numeric value: an explicit server assignment or a new serialized document is sufficient to exercise that input path.

## Worked code-judo proposal

The head's offset arithmetic followed by UTC formatting is effectively obtaining the original date's local calendar fields for the modern dates tested. The first simplification is therefore:

```ts
function local_calendar_date(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate())
}
```

The probe asserts this yields the same calendar result as the head for every matrix case. It removes input mutation, offset arithmetic, ISO conversion, slicing, splitting, and numeric reparsing. It is a behavior-preserving simplification for the tested domain, but it deliberately also preserves the regression. Shipping that simplification alone would fail the review.

The complete fix must retain the raw input until the conversion chooses its calendar fields. A worked boundary for the observed wire forms is:

```ts
type DatePickerDate = number | string

function date_from_value(value: DatePickerDate): Date {
  const date = new Date(value)
  const utc = typeof value === "number" || /^\d{4}-\d{2}-\d{2}$/.test(value)
  return utc
    ? new Date(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate())
    : new Date(date.getFullYear(), date.getMonth(), date.getDate())
}
```

Numeric Python dates use UTC fields. Existing selection strings use local fields. The exact date-only ISO shape also uses UTC fields, preserving a form that the base already handles correctly. This is a representation decision in one canonical conversion, not a collection of timezone exceptions. Pass raw values to this operation for the initial value and both limits; keep the existing null checks around optional limits. Reflect numeric-or-string runtime values in the affected property declarations.

This proposal was executed as plain JavaScript in the scratch matrix. It was not applied to the checkout or compiled as TypeScript. Its supported shapes must be documented, and additional date-time string forms should be evaluated if they are part of the supported JavaScript API. The matrix establishes correctness for Python numeric dates, `toDateString()` selections, and exact date-only ISO strings; it does not establish a universal date parser.

Keep `_on_select`'s existing callback representation. Changing it to a number could affect CustomJS consumers and Python's `Date.transform()` numeric path, which uses `datetime.date.fromtimestamp` in the server's local timezone. A global serializer change is therefore a larger compatibility exercise than this fix requires. A pure widget-level converter addresses the actual boundary without a new dependency.

Every current production call passes a fresh `Date`, so the added mutation has no demonstrated shared-object corruption at these call sites. Mutation is removable incidental complexity here, not a separate correctness finding. The string-only TypeScript declaration and runtime `p.Any` also predate the PR; they explain the hidden contract and should be corrected as part of F1 rather than reported as a second introduced defect.

## Verification limits and next action

Source inspection establishes how Python serializes the date and where the helper's outputs enter Pikaday. Node execution verifies the actual calendar conversion and the worked proposal in five zones. A Bokeh build, dependency installation, the browser suite, Pikaday runtime execution, and websocket execution were unavailable and were not attempted. No network was used.

Apply the boundary fix in a separate implementation task, add the coverage described in `02_datepicker_tests.md`, and run the focused project checks when the browser/build environment is available. The review checkout remains unchanged.
