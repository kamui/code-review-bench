# Thermo Nuclear Review — bokeh/bokeh#9232

Range reviewed: `ccb4bcb4c2b841d89b0e88303a97bf4604a5795f..36549bca3a63d581f7b68d08054a7813c1e6a499` (`git diff main...review-head`), two files: `bokehjs/src/lib/models/widgets/date_picker.ts` (+6/−2) and the new `tests/integration/widgets/test_datepicker.py` (+98).

The change rewrites `DatePickerView._unlocal_date` so that, before reading the ISO date string, it shifts the incoming `Date` by the browser's own timezone offset. The intent is to fix issue #9129 (users in UTC+ zones see the picked day minus one after selecting a date). I traced both ways a `Date` reaches this function and executed the old and new implementations under node with `TZ` set to eight zones. The fix is correct for one input path and introduces a regression of the same shape on the other.

## Findings

### Finding 1 — High — `_unlocal_date` now displays the wrong day (one day early) for every user west of UTC when the value comes from Python

File: `bokehjs/src/lib/models/widgets/date_picker.ts`, lines 82–83 (new shift), as consumed by lines 68–71.

Evidence traced:

- `render()` (lines 66–73) calls `_unlocal_date(new Date(this.model.value))`, and the same for `min_date` and `max_date`. `model.value` can hold two different encodings:
  1. A **number of milliseconds** when the value was supplied from Python. `bokeh/util/serialization.py` lines 178–184 convert both `datetime.datetime` and `datetime.date` as `obj - DT_EPOCH` with `DT_EPOCH = datetime.utcfromtimestamp(0)` (line 83), i.e. the date is encoded as **UTC midnight**. `DATETIME_TYPES` (lines 63–68) includes `dt.date`, and `BokehJSONEncoder.transform_python_types` (`bokeh/core/json_encoder.py` lines 185–186) routes all of them through `convert_datetime_type`. This is the encoding on initial page load and on every server-side assignment to `dp.value` / `dp.min_date` / `dp.max_date`.
  2. A **`toDateString()` string** such as `"Mon Sep 16 2019"` written by `_on_select` (line 95) after the user picks a day, and the JS-side default (line 124). `new Date("Mon Sep 16 2019")` is **local midnight**.
- The old code read `toISOString()` directly. That is right for encoding 1 everywhere (UTC midnight → UTC date components) and wrong for encoding 2 in UTC+ zones (local midnight is the previous UTC day). That is exactly issue #9129, and why UTC− users never saw it.
- The new code (lines 82–83) first subtracts `getTimezoneOffset()` minutes, which turns a local-time `Date` into one whose UTC components equal the local wall-clock components. That is the right transform for encoding 2 and makes the `toDateString()` path correct in every zone. But it is applied unconditionally, so it is **also applied to encoding 1**, which was already in UTC. For a browser at UTC−N the UTC-midnight timestamp is pushed back by N hours into the previous UTC day, and `toISOString().substr(0,10)` yields the day before.
- Node reproduction (scratch script, not in the clone), `Date.UTC(2019, 8, 20)` is what Python sends for `datetime(2019, 9, 20)` / `date(2019, 9, 20)`:

  | TZ | old `_unlocal_date` on Python ms | new `_unlocal_date` on Python ms | new on `"Mon Sep 16 2019"` |
  | --- | --- | --- | --- |
  | UTC | Fri Sep 20 2019 | Fri Sep 20 2019 | Mon Sep 16 2019 |
  | Europe/Paris (+2) | Fri Sep 20 2019 | Fri Sep 20 2019 | Mon Sep 16 2019 |
  | Pacific/Auckland (+12) | Fri Sep 20 2019 | Fri Sep 20 2019 | Mon Sep 16 2019 |
  | America/New_York (−4) | Fri Sep 20 2019 | **Thu Sep 19 2019** | Mon Sep 16 2019 |
  | America/Los_Angeles (−7) | Fri Sep 20 2019 | **Thu Sep 19 2019** | Mon Sep 16 2019 |
  | Pacific/Honolulu (−10) | Fri Sep 20 2019 | **Thu Sep 19 2019** | Mon Sep 16 2019 |

  The same off-by-one occurs for UTC-midnight inputs at every DST boundary date I tried in America/Los_Angeles (Mar 10, Mar 31, Oct 27, Nov 3 2019 all come back one day early). The string path was correct in all zones and on all DST boundaries tested.

Failure caused: for every browser in the Americas (and any other UTC− zone), the DatePicker now shows the **wrong initial date** on first render, and the wrong `minDate`/`maxDate` bounds, whenever those were set from Python — which is the normal way the widget is used (the issue's own reproduction, `DatePicker(value=datetime.date.today())`, will show yesterday for a US user). Any server-side callback that assigns `dp.value = some_date` re-renders through the same path and also displays the previous day. `min_date=datetime(2019, 9, 1)` becomes Aug 31 and lets the user pick a day that the Python model said was out of range. The model's Python-side `value` is not itself altered by this (the displayed value and `model.value` disagree, as in the original issue), so this is a direct mirror of #9129 moved from UTC+ users to UTC− users, who were previously unaffected. The change fixes a widely reported bug for one half of the world by introducing the identical bug for the other half; it should not ship as-is. The two encodings need to be distinguished (e.g. read UTC components for a numeric `model.value` and local components for a string one, or normalise what `_on_select` stores), rather than applying one shift to both.

Note on review of the author's reasoning: the PR thread records testing in PST as "working great". The selection path (what #9129 describes, and what the new tests exercise) is indeed correct in PST under the new code; the regression is only visible when checking the date shown on initial load or after a server push, which was not described as tested.

### Finding 2 — Low — The new integration tests pass with or without the fix on a UTC CI machine, and do not check the initially displayed date (the path that regresses)

File: `tests/integration/widgets/test_datepicker.py`, lines 42–49 (`test_basic`) and 51–68 (`test_js_on_change_executes`), 70–98 (`test_server_on_change_round_trip`).

Evidence traced: `test_basic` only asserts the label text and the absence of console errors; nothing reads the `input` value after the first render. The two selection tests click day 16 and assert `"Mon Sep 16 2019"`. From the node table above, in TZ=UTC the old and new `_unlocal_date` return identical results for both encodings, so on the project's CI (which runs in UTC) these assertions pass against the pre-fix code too. In a UTC− zone they would also pass against both versions. The only zone in which the tests distinguish old from new is UTC+, which CI does not run in. The round-trip test compares the Python-side timestamps via `utcfromtimestamp`, which is timezone-neutral and also never touches the displayed value.

Failure caused: no automated guard for the bug being fixed, and no coverage for Finding 1 — an assertion that `.bk-input` shows `"Fri Sep 20 2019"` immediately after `bokeh_model_page(dp)` would have failed in any UTC− zone under the new code. The PR checklist item "tests added / passed" is left unchecked, consistent with this. This does not break anything by itself, hence Low.

## Checked and not reported

- `date.setTime(...)` mutates its argument. All three callers pass a freshly constructed `Date` (lines 68, 70, 71), so nothing else observes the mutation. Not an issue.
- Re-rendering after a selection recreates the input; `LayoutDOMView.render` empties `this.el` first (`layout_dom.ts` line 115), so the unscoped `.bk-input` selector in the tests matches exactly one element. Fine.
- All fixtures and helpers referenced by the new test file exist (`bokeh_model_page`, `bokeh_server_page`, `RECORD`, `click_custom_action`, `has_no_console_errors`, `results`; `integration` and `selenium` markers are registered in `setup.cfg`). The boilerplate matches `test_radio_button_group.py`.
- No security, devex, or feature-gate implications: the change is a pure client-side date computation and a test file; no dependencies, environment, build steps or network surfaces change.
