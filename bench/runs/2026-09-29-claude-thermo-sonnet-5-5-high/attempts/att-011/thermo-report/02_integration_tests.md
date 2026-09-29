# Detail 02 — `tests/integration/widgets/test_datepicker.py` (new, 98 lines)

Selenium integration tests could not be run here (no browser, no network, no build). All statements below are from reading and from node-based reasoning about the JS under test.

## Finding 3 — the tests do not exercise the bug being fixed (verified by reading)

The PR's purpose is to make the displayed date correct across time zones, yet no test sets or varies the zone and no test asserts the initially rendered date.

- `test_basic` (test_datepicker.py:42-50) only checks the label text; it would pass on any `_unlocal_date` implementation, including a broken one.
- `test_js_on_change_executes` (lines 52-70) does assert the input text after a pick (line 68) but that is the path that is correct in UTC and in all UTC- zones with the *original* code, and its result depends on the browser's zone. Run in UTC, which is the CI default, it passes both before and after the fix. It therefore does not detect issue #9129 in CI and cannot detect the UTC- regression from detail 01, which lives on the initial-render path. The clicked cell `data-pika-day="16"` also silently depends on Pikaday opening on September 2019, i.e. on `_unlocal_date` of the initial value being right, without ever asserting it.
- `test_server_on_change_round_trip` (lines 72-98) validates Python-side old/new values decoded with `utcfromtimestamp`, which checks serialization, not display.

Remedy: add an assertion on `.bk-input` immediately after render (before any click) that the text equals `Fri Sep 20 2019`, and run the module under several zones. The maintainer suggested varying the zone; Chromium honours the `TZ` environment variable, so a parametrized fixture that sets `TZ` for the driver process (UTC, Europe/Paris, America/Los_Angeles, Pacific/Auckland) is the direct way, or Selenium/CDP `Emulation.setTimezoneOverride`. Both the initial-display assertion and the post-pick assertion should run in every zone. Status: recommendation; would have failed for `America/Los_Angeles` on the PR head according to the node measurements.

## Finding 4 — repeated setup and a clock-dependent bound (verified by reading)

The same 150-character `DatePicker(...)` construction is copy-pasted three times (lines 43, 53, 78), and `max_date=datetime.utcnow()` makes each test depend on wall-clock time: near midnight UTC the bound moves a day, and `max_date` is also fed through the function under repair, so the assertions are not hermetic. Use a module-level factory (or fixture) with fixed dates, e.g. `max_date=datetime(2019, 9, 30)`, and reduce each test to what it varies. The `find_element` + `click` + `data-pika-day` pick sequence is likewise duplicated in two tests (lines 58-62 and 86-90) and belongs in one helper.

## Minor

- `# External imports` is an empty section (line ~ 24); harmless, matches repo boilerplate.
- The PR body leaves "tests added / passed" unchecked although tests were added later by the maintainer; not a code issue.
