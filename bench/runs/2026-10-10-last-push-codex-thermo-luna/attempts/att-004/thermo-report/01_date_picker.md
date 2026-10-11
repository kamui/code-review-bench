# DatePicker date normalization

## Scope and structural assessment

The production change adds four lines to the existing `_unlocal_date` helper, and the new test module adds 98 lines. No file approaches the 1,000-line threshold, and the diff adds no branching or new module layer. The maintainability concern is the conversion contract: one helper is given a `Date` created from values with different semantics, but it applies the same offset correction to both.

`render()` passes `new Date(this.model.value)`, `new Date(this.model.min_date)`, and `new Date(this.model.max_date)` to `_unlocal_date` (lines 68-71). Python `Date` values are serialized as milliseconds since the Unix epoch; a date such as 2019-09-20 is therefore represented by UTC midnight. Separately, `_on_select()` stores `date.toDateString()` (line 95), which is a local date string and is reparsed as a local date when the view renders again. Those values require different interpretation if the implementation keeps both representations.

## Finding 1: Offset adjustment shifts server timestamps in UTC− zones

**Evidence.** `bokehjs/src/lib/models/widgets/date_picker.ts:82-87` obtains `getTimezoneOffset()`, subtracts it from the supplied date, and extracts the resulting UTC date with `toISOString()`. For a UTC-midnight timestamp at 2019-09-20 in `America/New_York`, `getTimezoneOffset()` is +240 minutes. Subtraction changes the timestamp to 2019-09-19 20:00Z, so the extracted day is September 19. The result is returned as a local `Date` for September 19. Since the same helper handles `value`, `min_date`, and `max_date`, bounds can be shifted too.

The issue is representational rather than a universal offset direction: the selected local string `Fri Sep 20 2019` parses to local midnight, and the adjustment brings it to UTC midnight. That fixes the reported UTC+ local-string round trip. It also means blindly removing the correction would leave that local-string round trip broken. The helper currently cannot know which interpretation to apply because callers erase the input representation by constructing `Date` first.

**Worked code-judo proposal.** Make the date-only contract explicit instead of adjusting an undifferentiated timestamp. Convert server-side epoch values to the intended calendar day using their UTC year/month/day fields, and convert local date strings using their local year/month/day fields; construct Pikaday's local `Date` directly from those components. A small conversion at the caller/model boundary can select the correct interpretation based on the original value shape. Better still, if the model API can be normalized compatibly, retain a canonical date-only representation and have both server serialization and browser selection use it. This removes the offset arithmetic, the mutating `setTime` call, and the ISO-string split from the shared helper. Avoid changing the passed-in `Date` as a side effect.

**Action.** Preserve server calendar dates in UTC− zones while retaining the selected-date round trip in UTC+ zones. Apply the same invariant to `value`, `min_date`, and `max_date`.

**Verification status.** Reproduced with the changed arithmetic in scratch Node scripts under `America/New_York` and `Europe/Paris`. The New York run mapped the UTC timestamp to September 19 and the local date string to September 20. The Paris run mapped both representations to September 20. This is a focused behavior reproduction, not execution of Bokeh's bundle or browser integration suite.

## Finding 2: The new integration coverage misses the conversion regression

**Evidence.** In `tests/integration/widgets/test_datepicker.py`, `test_basic` (lines 42-50) verifies only the title and console errors. `test_js_on_change_executes` and `test_server_on_change_round_trip` select September 16 before asserting values (lines 52-70 and 72-98). None asserts the initial displayed September 20 date, checks the min/max boundary days, or configures and verifies browser behavior in multiple timezones. In particular, a test that only selects a day exercises `_on_select()` and the local date-string path; it does not show that the initial UTC timestamp still maps to the right day.

**Worked code-judo proposal.** Keep the callback tests for client and server API behavior, and add a narrowly focused test for the date-only conversion contract. Cover an initial UTC-midnight value plus UTC-midnight min/max values and a selected local date string. Run that case with a positive and negative UTC offset, or test a pure conversion helper with explicit timestamp and local-calendar inputs if controlling Selenium's timezone is not practical. Assert the visible initial date and the enabled/disabled edge days so both the displayed value and bounds are protected.

**Action.** Add assertions that fail for a one-day shift in initial value or either bound under UTC− and UTC+ configurations. Keep timezone and input-shape cases explicit so the regression remains easy to diagnose.

**Verification status.** The test source was inspected but not run. The task packet marks the Selenium suite, browser, and project build unavailable. The scratch Node reproduction above demonstrates the missing distinction, but it does not replace an integration run.

## Commands and measurements

- `git diff --stat main...review-head` reported two files changed, 104 insertions and 2 deletions.
- `git diff --check main...review-head` reported no whitespace errors.
- `wc -l` reported 129 lines in `date_picker.ts` and 98 lines in `test_datepicker.py`.
- `rg` and direct reads traced Python date serialization and the DatePicker call sites; Python date values are converted to epoch milliseconds, and the JS renderer reconstructs them with `new Date(...)`.
- Scratch Node commands ran the committed arithmetic under the host's `America/New_York` timezone and under `TZ=Europe/Paris`.
- The checkout was not modified. Selenium/browser/build execution was unavailable per policy.
