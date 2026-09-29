# Thermo-nuclear code quality review — bokeh/bokeh#9232 (DatePicker UTC+ display fix)

Range: `ccb4bcb4c2b841d89b0e88303a97bf4604a5795f..36549bca3a63d581f7b68d08054a7813c1e6a499` (`git diff main...review-head`). Reviewed by a single reviewer, model `claude-sonnet-5-5` at `high`; no other model or reviewer was used. Scope: 2 files (+104/−2). No file crosses 1000 lines (`date_picker.ts` is 130 lines), so the size gate is not triggered. Selenium tests and the bokehjs build were not runnable; the JS behaviour was checked with scratch node scripts under several `TZ` values.

## Verdict

Not approvable as is. The production change is a local patch over a design problem: `_unlocal_date` is asked to interpret two differently-encoded inputs (UTC-midnight timestamps from Python, local-midnight strings written by the widget's own `_on_select`), and the PR picks a compensation that is right for one of them. Measured with node, it cures the UTC+ case in the issue and introduces the mirror-image bug for UTC- users, and the new integration tests are not able to see either. There is a clear code-judo move that makes the function trivially correct in every zone.

## Findings

**1. The fix moves the bug from UTC+ to UTC- instead of removing it** (`bokehjs/src/lib/models/widgets/date_picker.ts:78-88`, detail file `01_date_picker_widget.md`, verified by execution). Python serializes a `date` as milliseconds at UTC midnight, so `new Date(value)` is a UTC-midnight instant, while after a click `_on_select` stores `toDateString()`, which reparses as local midnight. The original ISO-slice was correct for the first and wrong for the second in UTC+ zones (issue #9129). The new offset shift at lines 82-83 is correct for the second and now wrong for the first in UTC- zones: with `TZ=America/Los_Angeles` or `America/Sao_Paulo`, `DatePicker(value=date(2019, 9, 20))` initially displays Thu Sep 19, and `min_date`/`max_date` (lines 70-71) shift by a day too, changing which days are selectable. The maintainer's PST check only covered the pick-then-re-render path. This is presumptively blocking: a correctness regression for a large user population in exchange for fixing another.

**2. Missed code-judo: normalize the value convention at the boundary rather than patching the converter** (same file, detail 01). The root cause is that the widget writes a different representation than Python does. If `_on_select` stored a UTC-midnight timestamp (`Date.UTC(y, m, d)`, same convention as the server), and the default value at line 124 did the same, `_unlocal_date` would become `new Date(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate())`: one input meaning, no offset arithmetic, correct in every zone by construction, and finding 1 disappears rather than being re-tuned. The fallback, if changing the stored format is too invasive, is an explicit type dispatch at the top of `_unlocal_date` that names the two producers.

**3. `_unlocal_date` now mutates its parameter and still round-trips through an ISO string** (date_picker.ts:82-87, detail 01, verified by reading). `date.setTime(...)` silently modifies the caller's `Date`; it is harmless only because current callers pass throwaway objects. The `toISOString().substr().split().Number()` chain exists only to read three calendar fields that `getUTC*` accessors expose directly. The new comment (lines 79-81) explains the arithmetic but not the invariant, which is why the UTC- regression escaped notice, and it deviates from file style (`//Get`, "systems's"). The judo change above removes all of this.

**4. The new integration tests cannot detect the bug they accompany** (`tests/integration/widgets/test_datepicker.py:42-98`, detail file `02_integration_tests.md`). No test varies the time zone, none asserts the initially rendered value, and `test_basic` asserts only the label. The one test that reads the input text (line 68) does so after a pick, which is the path that already worked in UTC (the CI zone) before the fix, so it passes with and without the change and would also pass with the UTC- regression from finding 1. The pick at `data-pika-day="16"` implicitly depends on the initial date being right without asserting it. Add an assertion on `.bk-input` right after render and parametrize the module across zones (Chromium honours `TZ`, or use CDP timezone override) covering at least UTC, Europe/Paris, America/Los_Angeles and Pacific/Auckland.

**5. Test setup is duplicated and clock-dependent** (`test_datepicker.py:43, 53, 78`, and the pick sequence at 58-62 / 86-90, detail 02). The same long `DatePicker(...)` construction appears three times, and `max_date=datetime.utcnow()` makes the bounds, which pass through the very function under repair, depend on wall-clock time. Use one factory with fixed dates and extract the click-a-day helper.

## Proposed remediation sequence

1. Choose UTC-midnight as the single canonical representation of `DatePicker.value` on the client, matching the Python serializer.
2. Change `_on_select`, the property default, and `_unlocal_date` to that convention; remove the mutation and the ISO-string round trip (findings 1-3).
3. Add zone-parametrized tests that assert the initial and post-pick display, with fixed dates and a shared factory/helper (findings 4-5). Confirm the new tests fail on the PR head under `America/Los_Angeles` and on the original code under `Europe/Paris`.
4. Tidy the PR metadata (tests checkbox) once the above lands.

## Where the evidence is

Full measurements, the node commands, the per-zone results table, and the worked judo proposal are in `01_date_picker_widget.md`. The test-coverage analysis and test-structure remedies are in `02_integration_tests.md`.
