# Date conversion

## Scope and measurements

The changed production code is `DatePickerView._unlocal_date` in `bokehjs/src/lib/models/widgets/date_picker.ts`. The method is called for `value`, `min_date`, and `max_date` as each is converted from the model value to a JavaScript `Date` before Pikaday is constructed. The patch adds two lines of behavior: it reads the browser-local offset and mutates the passed date by subtracting that offset.

The model boundary matters here. `DatePicker.value`, `min_date`, and `max_date` are Python `Date` properties. In `bokeh/util/serialization.py`, `convert_datetime_type` converts a Python `datetime.date` to milliseconds from a naive UTC epoch, so a date such as 2019-09-20 is represented as `2019-09-20T00:00:00.000Z`. The new code treats that date-only UTC representation as an instant whose local offset should be applied. In `America/Los_Angeles`, `getTimezoneOffset()` is positive; subtracting seven hours from midnight UTC changes the ISO date to September 19. In `Europe/Paris`, the offset is negative in September, so the arithmetic moves forward and keeps September 20. The result is asymmetrical across the zones this widget must support.

## Finding

### The new offset adjustment shifts UTC-midnight dates to the previous day in western time zones

`date_picker.ts:82-83` mutates the serialized timestamp using the browser's local offset before extracting its ISO date. I ran the changed arithmetic in Node with `TZ=America/Los_Angeles` for `2019-09-20T00:00:00.000Z`; after subtraction, the extracted date was `2019-09-19`. The same input under `TZ=Europe/Paris` remained `2019-09-20`. The former result will make the initial selection, minimum date, or maximum date one day early for western users. This change therefore introduces a clear regression while attempting to correct another zone's display issue.

The current operation also mutates its `Date` argument, which obscures the conversion contract and would surprise a future caller that reuses the value. Each current call site passes a fresh `Date`, so mutation is not itself a present call-site defect; it is avoidable statefulness in a conversion that should be a pure mapping from a serialized calendar date to Pikaday's local-midnight date.

## Worked code-judo proposal

Keep the distinction between the model's UTC date and Pikaday's local `Date`, but do not translate between them by changing the timestamp. Read the UTC year, month, and day from the incoming date, then construct the local date directly from those components:

```ts
_unlocal_date(date: Date): Date {
  return new Date(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate())
}
```

This removes the offset multiplication, timestamp mutation, ISO string formatting, split, and numeric parsing. It directly expresses the invariant the method needs: retain the date components encoded at the model boundary while giving Pikaday a local `Date`. Verify behavior for timestamps and strings supported by the model before adopting it, since the public property accepts several date representations and the existing component tests do not cover those input forms.

## Verification status

- Inspected the full committed diff and the call sites in `DatePickerView.render`.
- Confirmed Python date serialization in `bokeh/util/serialization.py` uses milliseconds since the UTC epoch.
- Ran the offset and ISO-date calculation in Node under `Europe/Paris`, `America/Los_Angeles`, `Pacific/Kiritimati`, and `UTC`. The concrete failure is `America/Los_Angeles`: UTC midnight on September 20 becomes September 19 after the patch's subtraction.
- `git diff --check main...review-head` produced no whitespace errors.
- The repository's Selenium integration suite and browser are unavailable under this task's execution policy, so the added integration tests were not run. They do not set a timezone; their fixed selected-day assertions cannot catch this regression.

## Action

Delete the offset mutation and preserve the model date's UTC calendar components. Add browser tests that exercise the initialized picker in both a UTC+ zone and a UTC− zone; assert the displayed selected date and both date bounds so the same conversion path is covered for all three inputs.
