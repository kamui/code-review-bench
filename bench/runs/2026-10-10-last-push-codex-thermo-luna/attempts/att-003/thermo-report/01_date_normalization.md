# Date normalization and regression coverage

## Finding

In `bokehjs/src/lib/models/widgets/date_picker.ts:82-83`, `_unlocal_date` mutates the supplied `Date` by subtracting `getTimezoneOffset()` and then extracts its ISO calendar date. The method is called for `value`, `min_date`, and `max_date` in `render()` at lines 68-71.

This method receives values with different semantics. The Python model declares these properties as `Date` (`bokeh/models/widgets/inputs.py:253-263`). `bokeh/util/serialization.py:152-176` converts Python `datetime` and `date` values to milliseconds from the Unix epoch, and these date values represent UTC midnight. After a user selects a day, `_on_select` instead stores `date.toDateString()` (`date_picker.ts:90-96`), a local date string which `new Date(...)` parses as a local date in the browser. The adjustment is appropriate to recovering the date components from that latter local-midnight value, but it is not appropriate to every `Date` passed to this method.

For a UTC− browser, `new Date(UTC-midnight)` falls on the previous local evening and has a positive `getTimezoneOffset()`. Subtracting that offset moves the instant further into the previous UTC date; `toISOString().substr(0, 10)` therefore yields the previous day. On initial render this can make the shown value, as well as the Pikaday min/max bounds, one day early. The same code path can behave differently after a user selection because the selected value is a local date string. This is a representation-boundary problem, not a need for another conditional scattered through `render()`.

The added integration tests at `tests/integration/widgets/test_datepicker.py:43-98` cover a title, a JavaScript selection, and a server callback round trip. They assert a selected value and callback timestamps, but do not assert the initial displayed date or bounds, and they do not vary the browser timezone. Consequently, the test coverage does not expose the UTC-midnight versus local-date-string distinction.

## Worked code-judo proposal

Make the conversion boundary accept the model value in its original representation rather than first erasing that distinction with `new Date(value)`. Normalize the value once into calendar components: derive components in UTC for an epoch-millisecond model value, and derive the date components from the local date representation used for selected values. Construct the local `Date(year, month, day)` required by Pikaday from those components. Route `value`, `min_date`, and `max_date` through the same normalization function, and leave the supplied value untouched.

This removes the implicit assumption that a timestamp and a local date string have the same timezone meaning. A single, explicit date-only boundary is simpler to reason about than mutating an instant based on the host timezone and then reparsing its ISO string. It also puts the normalization beside the model-to-Pikaday conversion where the mixed representations enter the widget.

## Evidence and review measurements

The committed change is 6 added and 2 removed lines in `date_picker.ts`, plus a new 98-line integration test file. The TypeScript file is 129 lines after the change, so no large-file threshold is implicated. The added path is compact; the concern is the implicit input contract and the broad application of the offset rather than file growth or branching complexity.

Commands used for inspection:

- `git diff --find-renames --unified=80 main...review-head -- bokehjs/src/lib/models/widgets/date_picker.ts tests/integration/widgets/test_datepicker.py`
- `nl -ba bokehjs/src/lib/models/widgets/date_picker.ts | sed -n '62,102p'`
- `nl -ba bokeh/models/widgets/inputs.py | sed -n '242,267p'`
- `sed -n '140,185p' bokeh/util/serialization.py`

## Verification status

Static review only. The serialization path and both JavaScript representations were inspected, and the UTC− behavior follows from `Date` offset semantics. No browser, Selenium integration suite, or project build is available in this run, and no tests were run. The remediation should be validated with the focused integration assertions under UTC+ and UTC− settings when those facilities are available.
