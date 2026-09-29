# DatePicker date normalization

## Finding: applying the local offset to UTC-midnight values shifts them to the prior day

The new logic in `bokehjs/src/lib/models/widgets/date_picker.ts:82-83` mutates every incoming `Date` by subtracting its local timezone offset before reading its ISO date. That rule conflates two date representations already used by this widget: a local date string emitted by `_on_select()` and UTC-midnight milliseconds sent from Python. The former needs conversion to recover the intended local calendar date in UTC+ zones; the latter already encodes the intended date in its UTC calendar components and must not receive that adjustment.

The Python serializer confirms the second representation: `bokeh/util/serialization.py:178-185` converts `datetime.date` to milliseconds from `DT_EPOCH` by constructing a midnight `datetime` and subtracting the UTC epoch. `DatePickerView.render()` wraps `model.value`, `min_date`, and `max_date` in `new Date(...)` at `bokehjs/src/lib/models/widgets/date_picker.ts:68-71`, so the changed helper also processes these epoch-backed values.

## Evidence and verification

- `git diff --no-ext-diff --unified=80 main...review-head -- bokehjs/src/lib/models/widgets/date_picker.ts tests/integration/widgets/test_datepicker.py` shows the unconditional offset adjustment and the newly added tests.
- `sed -n '155,205p' bokeh/util/serialization.py` shows Python `date` serialization as milliseconds from UTC epoch midnight.
- A focused Node reproduction under `TZ=America/Los_Angeles` passed `Date.UTC(2019, 8, 20)` through the added operation. The original ISO date was `2019-09-20`; after subtracting `getTimezoneOffset() * 60000`, it was `2019-09-19`. Under `TZ=Europe/Paris`, the same UTC-midnight input remained `2019-09-20`, which explains why testing only affected UTC+ zones misses the regression.
- The new integration tests initialize `DatePicker` from Python `datetime` values but do not assert the displayed initial date or run under multiple timezone settings. The focused reproduction verifies the arithmetic; the Selenium suite and browser were unavailable, so the integration tests were not run.

## Worked code-judo proposal

Represent the widget's date-only value explicitly at the conversion boundary instead of trying to infer its intended calendar day after converting all inputs to `Date`. Preserve the source form while normalizing: for epoch-backed numeric values, take `getUTCFullYear()`, `getUTCMonth()`, and `getUTCDate()`; for the local date string emitted by `_on_select()`, take the corresponding local calendar components. Construct the Pikaday date from those components. This removes the blanket timezone mutation and makes the date-only invariant visible in the helper's input contract. If practical, centralize that normalization for `value`, `min_date`, and `max_date` so all three properties share the same explicit policy.

## Actionable remediation

Change the helper boundary to retain enough information to distinguish the epoch-backed values from local date strings, then add cases for both representations in a UTC− timezone and UTC+ timezone. Verify the initial selected date and min/max limits as well as the date selected and returned by the picker. Do not apply the offset correction to UTC-midnight values.
