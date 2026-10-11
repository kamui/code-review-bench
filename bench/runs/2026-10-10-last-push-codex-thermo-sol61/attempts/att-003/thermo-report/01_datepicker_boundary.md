# DatePicker conversion boundary

## Judgment

The change repairs one date representation by imposing its timezone semantics on another. Request changes on the offset adjustment at `bokehjs/src/lib/models/widgets/date_picker.ts:82–83`. The strongest structural remedy is to stop erasing the representation before interpreting the date. DatePicker can construct the local calendar date from the appropriate fields, without modifying the input timestamp, formatting it as ISO, splitting a string, and parsing three numbers.

This is a boundary defect with a demonstrated behavioral regression, rather than a request to introduce a general date framework. Keep the logic in the widget that owns the date-only meaning. Changing the generic serializer for all date-bearing models would unnecessarily enlarge the scope.

## Source evidence

`DatePickerView.render()` at lines 66–73 creates `new Date(this.model.value)` before calling `_unlocal_date()`. It performs the same conversion for `min_date` and `max_date`. By the time the helper runs, a numeric UTC wire date and a parsed local calendar string are indistinguishable `Date` instances.

Python's `DatePicker` declares all three properties as `Date` in `bokeh/models/widgets/inputs.py:253–263`. `Date.transform()` in `bokeh/core/property/datetime.py:58–69` parses Python-side string values into dates. `BokehJSONEncoder.transform_python_types()` in `bokeh/core/json_encoder.py:184–186` delegates recognized dates to `convert_datetime_type()`. That function, in `bokeh/util/serialization.py:178–184`, represents naive datetimes and dates as milliseconds from the UTC epoch without applying a browser timezone. A Python `date(2019, 9, 20)` therefore reaches the browser as `1568937600000`, the instant `2019-09-20T00:00:00.000Z`.

In contrast, `_on_select()` at `date_picker.ts:90–96` assigns `date.toDateString()` to `model.value`. That string represents the selected local calendar day. Parsing `Fri Sep 20 2019` in Paris produces the instant at local midnight, which is still September 19 in UTC. The base helper incorrectly selects the UTC date of that instant. The head's offset correction repairs this local-string path.

The same correction is wrong for UTC-midnight wire inputs. In Los Angeles on September 20, `getTimezoneOffset()` is 420 minutes. Subtracting 25,200,000 milliseconds moves the UTC-midnight input to `2019-09-19T17:00:00.000Z`. The subsequent ISO truncation yields September 19, and the final constructor creates local midnight on September 19. The base helper selected the original September 20 UTC fields correctly.

For the September 1 minimum used by the added tests, the head supplies August 31 to Pikaday in that timezone. A fixed September 30 maximum likewise becomes September 29. This follows directly from the arguments at render lines 70–71. The actual Pikaday DOM and enabled-button behavior were not executed here, so the finding distinguishes the reproduced conversion error from its inferred UI consequences.

## Measurements and commands

Read the change with `git diff main...review-head` and the numbered source with `nl -ba bokehjs/src/lib/models/widgets/date_picker.ts`. Read the corresponding Python property, serializer, and JSON encoder sources. A scoped `rg` search found no existing equivalent date-only conversion utility in the inspected BokehJS core and widget source; the conversion should remain owned by DatePicker rather than duplicate or alter the Python wire serializer.

`git show main:bokehjs/src/lib/models/widgets/date_picker.ts | wc -l` reports 125 lines. `wc -l` reports 129 lines at head. The helper grows from an ISO-string conversion into an offset-mutation-plus-ISO conversion. No conditional branches are added by the PR. The concern is the semantic contract and avoidable transformations, not large-file growth or branching volume.

The scratch harness is at `../datepicker-check.cjs`, outside the checkout. It reads the head helper body using `fs.readFileSync()` and executes that body without its TypeScript signature. It contains the verbatim three-statement base conversion and a candidate raw-value conversion. Run it from the work directory with:

```sh
node --version
for review_tz in UTC America/Los_Angeles Europe/Paris Asia/Kolkata Pacific/Auckland America/St_Johns; do
  TZ="$review_tz" node datepicker-check.cjs
done
```

Node reports `v24.21.0`. Each command completed in under a second; no browser, package installation, project build, fixture server, or network was used.

The matrix uses September 1, 16, and 20; January 20; March 31; and November 3, all in 2019. The last two exercise dates on common DST transition days. For each calendar date, inputs are a UTC-midnight numeric wire value, a locally constructed date's `toDateString()` result, and an ISO date-only string. The ISO case checks an additional string representation accepted by `new Date()` at the current JavaScript boundary; the Python path itself produces numeric wire values.

| Timezone | Cases | Base failures | Head failures | Candidate failures |
| --- | ---: | ---: | ---: | ---: |
| UTC | 18 | 0 | 0 | 0 |
| America/Los_Angeles | 18 | 0 | 12 | 0 |
| Europe/Paris | 18 | 6 | 0 | 0 |
| Asia/Kolkata | 18 | 6 | 0 | 0 |
| Pacific/Auckland | 18 | 6 | 0 | 0 |
| America/St_Johns | 18 | 0 | 12 | 0 |

For the concrete September 20 case:

| Timezone and representation | Expected local day | Base | Head |
| --- | --- | --- | --- |
| Los Angeles, UTC wire number | 2019-09-20 | 2019-09-20 | 2019-09-19 |
| Los Angeles, local selection string | 2019-09-20 | 2019-09-20 | 2019-09-20 |
| Paris, UTC wire number | 2019-09-20 | 2019-09-20 | 2019-09-20 |
| Paris, local selection string | 2019-09-20 | 2019-09-19 | 2019-09-20 |

The harness also observes input mutation: minus 25,200,000 milliseconds in Los Angeles for the numeric September 20 input. Current render callers allocate disposable Dates, so this mutation is not reported as an independent aliasing defect. Removing it makes the conversion easier to reason about without overstating its present impact.

## Worked code-judo proposal

Move representation interpretation in front of Date construction. For the proven numeric wire and local selection paths, plus ISO date-only strings, the boundary can look like this:

```ts
type PickerDateValue = number | string

_picker_date(value: PickerDateValue): Date {
  const date = new Date(value)
  const utc_fields = typeof value === "number" ||
    /^\d{4}-\d{2}-\d{2}$/.test(value)

  if (utc_fields)
    return new Date(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate())

  return new Date(date.getFullYear(), date.getMonth(), date.getDate())
}
```

Call this with `this.model.value`, `this.model.min_date`, and `this.model.max_date` directly, retaining the current null guards for optional bounds. The existing `Props` declarations say `string`, but runtime Python values are numbers; align the affected declarations with the raw-value boundary when implementing it. This is evidence about the existing mixed representation, not a separate finding against pre-existing typings.

The small dispatch expresses the actual protocol distinction once at the owning boundary. It replaces the caller's representation-erasing Date construction and the helper's arithmetic, mutation, ISO formatting, string slicing, splitting, and numeric parsing with direct calendar-field construction. It needs no external dependency or shared serializer changes.

Keep `_on_select()` emitting `toDateString()` so existing JavaScript callbacks still receive the current string representation. Setting the model to a UTC number or ISO string might simplify the internal model further, but would change an observable callback contract demonstrated by the new tests. That is not a behavior-preserving remedy for this PR.

The code is a worked proposal, not an applied or compiled patch. Its JavaScript counterpart passed the 108-case matrix. It covers the exercised modern dates and input forms; additional direct JavaScript inputs, such as strings carrying an explicit timezone or time of day, need an explicit date-only interpretation when finalizing the boundary. Existing behavior for invalid dates, historical years, null default values, and the Pikaday positioning patch was not changed or audited as new defects.

## Verification status and remediation

The conversion regression is reproduced offline using the actual head helper. The Python-to-wire provenance is established from source. Effects on picker arguments are established from their call sites; rendered behavior and browser/server synchronization remain unverified because the permitted environment supplies neither dependencies nor a browser.

Implement the raw-value boundary, retain public selection output, and verify the initial date and both endpoint arguments against the deterministic matrix. Then run the browser checks discussed in `02_datepicker_tests.md`. Do not accept a sign flip or another timezone adjustment in the existing helper: it still receives two representations whose intended calendar fields differ.
