# 02 — New integration tests (tests/integration/widgets/test_datepicker.py)

Scope: the new 98-line Selenium test module added by the PR (commits `4215c2d0b` and `36549bca3`). It contains three tests: `test_basic` (line 42), `test_js_on_change_executes` (line 52) and `test_server_on_change_round_trip` (line 72).

Execution status: **not run**. The Selenium suite, a browser and the project build are unavailable in this review. Everything below comes from reading the tests against the helper behaviour verified in 01 (`clone-work/scratch/unlocal.js`).

## Finding 2.1 — The tests cannot distinguish the fix from the bug, and never look at the display path the fix regresses

The PR adds the first tests this widget has ever had, which is good. But none of them pins the behaviour the PR changes.

- The node sweep in 01 shows that under `TZ=UTC` the `main` and `review-head` versions of `_unlocal_date` return identical results for every input shape. Nothing in the harness pins a browser timezone (`git grep -i "TZ\b|timezone"` over `bokeh/_testing`, `tests/integration`, `ci/`, `scripts/` and `.travis.yml` finds only unrelated hits in `scripts/issues.py`). On a UTC CI runner all three tests pass on `main` too, so they are not regression tests for #9129.
- The one assertion that does exercise the #9129 path is the re-rendered input text at lines 67-68 (`el.get_attribute('value') == 'Mon Sep 16 2019'`). It only fails on the old code when the browser runs in a UTC+ zone.
- No test asserts the **initial** displayed value produced from the Python `value=datetime(2019, 9, 20)`. That is exactly where the patch regresses UTC− users (01, Finding 1.1): in America/Los_Angeles the initial input would read "Thu Sep 19 2019". `test_server_on_change_round_trip` asserts `d0 == (2019, 9, 20)` at line 96, but `d0` is the *Python-side* old value echoed back through the data source. It never passes through `_unlocal_date` and says nothing about the display.

So the suite adds three browser tests and still leaves the changed function's contract, "the same calendar day in every zone, for both input encodings", completely unpinned.

Remedy. The cheapest and strongest pin is not a Selenium test at all. Once the conversion is the pure `calendar_date(value)` function proposed in 01, a BokehJS unit test can assert that `calendar_date(Date.UTC(2019, 8, 20))` and `calendar_date("Fri Sep 20 2019")` both give local Sep 20. The dispatch makes that correct by construction in any zone, and running the unit suite under one extra `TZ` (for example `TZ=America/Los_Angeles` and `TZ=Asia/Tokyo`) makes it observable. On the integration side, `test_basic` should also assert the initial input text (`.foo input` value `== 'Fri Sep 20 2019'`). That is a one-line addition that catches the UTC− regression whenever the suite runs west of Greenwich.

## Finding 2.2 — Triplicated, time-dependent widget construction

Lines 43, 53 and 78 repeat the same 150-character `DatePicker(...)` constructor verbatim, and each uses `max_date=datetime.utcnow()`. The upper bound is wall-clock dependent, so the fixture changes every day the suite runs. It is also a `datetime` with a time of day, which 01 shows is itself shifted across a day boundary in far-east zones by the new helper. The tests only need a bound after Sep 20 2019. A module-level factory such as `def _make_picker(): return DatePicker(title='Select date', value=date(2019, 9, 20), min_date=date(2019, 9, 1), max_date=date(2019, 10, 31), css_classes=["foo"])` removes the triplication, makes the fixture deterministic, and uses `date` (the property's actual type) rather than `datetime`. Status: reading only; low severity.
