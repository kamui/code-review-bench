# 02 — `tests/integration/widgets/test_datepicker.py`

Scope: new file (+98). This covers three Selenium tests: `test_basic` (`:42-50`),
`test_js_on_change_executes` (`:52-70`) and `test_server_on_change_round_trip` (`:72-98`).
The Selenium suite and a browser are unavailable in this run, so nothing below was executed. The
conclusions come from reading the test code alongside the timezone behaviour verified in
`01_bokehjs_date_picker.md`.

## Finding 2.1 — The tests cannot detect #9129 or the regression this PR introduces

**Severity: high (the PR's safety net does not cover the change it ships). Status: verified by
reading. The timezone behaviour it depends on is verified by execution in detail file 01.**

The defect is purely timezone-dependent. In UTC, both the pre-PR and the post-PR `_unlocal_date`
return the correct day for every input shape (first block of the `unlocal.js` output in 01).
Nothing in this file sets or varies the browser timezone. The tests therefore run in whatever
zone the CI host uses, normally UTC, and would pass unchanged against the pre-fix code. They are
not regression tests for #9129.

They also do not observe the regression from Finding 1.1. No test asserts what the input shows
**on initial render**. `test_basic` checks only the label text (`:47-48`).
`test_js_on_change_executes` checks the input text only after a click (`:68-69`). After a click,
the value is a `toDateString()` string, which the new code handles correctly everywhere. The broken
path is the initial render of a Python timestamp in a UTC− zone, and it has no assertion at all.

The code under test is also a hard place to test well. The conversion is buried in a view method,
so the only way to exercise it is to drive a full browser.

Remedy: pull the conversion into a pure, exported function, as proposed in
01 §1.1 (`to_local_date` / `to_iso_date`). Unit-test it in `bokehjs/test/models/widgets/`,
which already hosts widget unit tests (`slider.ts`, `paragraph.ts`, ...). Use a small table of
(input shape × expected calendar day), and run that suite under at least one UTC+ and one UTC−
`TZ` (for example `TZ=Europe/Paris` and `TZ=America/Los_Angeles`). That makes the contract
explicit and cheap to check, and it would have failed on both the original code and this PR. For
the Selenium layer, add one assertion that the rendered input text equals the Python-supplied
initial value, ideally with the browser launched under a non-UTC `TZ`.

## Finding 2.2 — Nondeterministic, triplicated fixture setup

**Severity: low (maintainability). Status: verified by reading.**

The same `DatePicker(title=..., value=datetime(2019, 9, 20), min_date=datetime(2019, 9, 1),
max_date=datetime.utcnow(), css_classes=["foo"])` construction is copy-pasted three times
(`:43`, `:53`, `:78`). It uses `max_date=datetime.utcnow()`, so the fixture changes every day the
suite runs. It also passes `datetime` objects to properties documented as "Date (but not DateTime)"
(`bokeh/core/property/datetime.py:51-53`). Those values only work because `datetime` subclasses
`date`, and they would feed a time-of-day component into exactly the conversion this PR is about.

Remedy: build the picker once in a module-level helper or fixture with fixed `datetime.date`
values, e.g. `max_date=date(2019, 10, 1)`. The copied `Copyright (c) 2012 - 2017` header (`:2`) is a
cosmetic leftover of the same copy-paste.
