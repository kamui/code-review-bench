# Detail 01 — DatePicker widget (`bokehjs/src/lib/models/widgets/date_picker.ts`) and integration test

Range reviewed: `git diff main...review-head` (2 files, +104/−2). `date_picker.ts` is 129 lines after the change, so there is no file-size concern.

## Finding 1 — The offset shift fixes UTC+ by breaking UTC− for Python-supplied dates (verified)

**Location:** `bokehjs/src/lib/models/widgets/date_picker.ts:78-88` (`_unlocal_date`), called at lines 68, 70 and 71.

**What the code does.** `_unlocal_date` now subtracts `getTimezoneOffset()*60000` from the instant, formats it with `toISOString()`, takes the first 10 characters, splits on `-`, and rebuilds a local `Date`. That is a shift, a string round trip, and a rebuild.

**Why it is wrong.** The model's `value`, `min_date` and `max_date` reach this function in two different encodings.

- Python-supplied dates are serialized by `bokeh/util/serialization.py:183-184` as epoch milliseconds at UTC midnight. They arrive in JS as a number, so `new Date(number)` is a UTC-midnight instant.
- Dates picked in the widget are stored by `_on_select` (line 95) as `date.toDateString()`, for example `"Fri Sep 20 2019"`. `new Date("Fri Sep 20 2019")` parses as local midnight.

The old code (read the UTC fields) was right for the first encoding and wrong for the second in UTC+ zones. That is the bug in #9129, which appears after the first pick. The new code (shift by the local offset, then read the UTC fields) is right for the second encoding and wrong for the first in UTC− zones. The PR fixes one encoding and breaks the other.

**Measurements.** I ran both versions of the function under node with `TZ` set. Command: `TZ=<zone> node t.js` (scratch script in the work tmp dir, not in the clone). Input encodings were `Date.UTC(2019,8,20)`, which is what Python sends, and `"Fri Sep 20 2019"`, which is what a pick stores. The expected result is `Fri Sep 20 2019`.

| TZ | input | old | new (PR) |
| --- | --- | --- | --- |
| Europe/Paris | UTC ms | Fri Sep 20 | Fri Sep 20 |
| Europe/Paris | local string | **Thu Sep 19** | Fri Sep 20 |
| America/Los_Angeles | UTC ms | Fri Sep 20 | **Thu Sep 19** |
| America/Los_Angeles | local string | Fri Sep 20 | Fri Sep 20 |
| Pacific/Auckland | UTC ms | Fri Sep 20 | Fri Sep 20 |
| Pacific/Auckland | local string | **Thu Sep 19** | Fri Sep 20 |
| UTC | all | correct | correct |

The ISO string `"2019-09-20"` (UTC parse) behaves like the UTC ms row.

**Consequence.** In any UTC− zone (the Americas), a Python-provided `DatePicker(value=date(2019,9,20))` now shows 19 Sep on first render, and so do `min_date` and `max_date`. The allowed range is shifted by one day, so the true boundary day can become unselectable. That is a regression of the common case, a date set from Python. The thread mentions testing in PST, but I cannot see what value the tester used. The issue's own reproduction uses `datetime.date.today()` from Python, which would hit the UTC ms path.

**Confidence:** the arithmetic is reproduced under node. How the current Bokeh JS serializer encodes `Date` properties is read from the Python source. I could not run a browser, so the end-to-end display is inferred, not observed.

**Code-judo proposal.** The real defect is that `_unlocal_date(new Date(x))` guesses an encoding after the fact. Delete the shift and the ISO string round trip, and normalize each encoding once, at the boundary:

```ts
// Python sends epoch-ms at UTC midnight; picks are stored as local toDateString() text
_to_local_date(v: number | string): Date {
  if (typeof v === "number") {
    const d = new Date(v)
    return new Date(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate())
  }
  return new Date(v)   // toDateString() text already parses as local midnight
}
```

I ran this variant under `TZ=UTC`, `Europe/Paris`, `America/Los_Angeles` and `Pacific/Auckland`. It gives `Fri Sep 20 2019` for both encodings in all four zones. It has no offset arithmetic, no `toISOString`/`substr`/`split` chain, and no mutation. The longer-term cleanup is to make `_on_select` and the Python-side serialization agree on one encoding. The existing `// XXX: this should be handled by the serializer` comment already points there.

## Finding 2 — `_unlocal_date` mutates its argument and layers a shift on a string round trip

**Location:** `date_picker.ts:82-83`.

`date.setTime(...)` mutates the caller's `Date`. The three call sites pass fresh temporaries, so there is no live bug today. But the method is named like a pure conversion and returns a different `Date`, so a mutated parameter plus a returned copy is a trap for the next caller. The function also has three steps where one would do: shift the instant, serialize it to text, then parse the text back into numbers. The shift exists only so that `toISOString()` prints the right local fields. Reading `getFullYear()`, `getMonth()` and `getDate()` directly on the right instant makes the whole function disappear (see the Finding 1 proposal).

The comments (lines 79-81) lack a space after `//`, which is inconsistent with the file and the rest of bokehjs. They also contain a typo ("systems's"). More importantly, they describe the mechanism but never say that two encodings arrive here. The original comment named the problem, "date comes in as a UTC timestamp and pikaday uses local", and the new comment dropped that explanation.

## Finding 3 — The integration test does not cover the regression it was added for, and cannot detect timezone behaviour

**Location:** `tests/integration/widgets/test_datepicker.py:38-98`.

- No test reads the initial displayed value. `test_basic` only checks the label text. Neither of the other two tests asserts what `.bk-input` shows before a click, and that is exactly where Finding 1's regression appears.
- The tests pass by clicking day 16 and reading the result. That path stores `toDateString()` text, which is the one the PR fixes, so the suite is green while the Python-supplied path is broken in UTC− zones.
- Nothing varies `TZ`. In a CI container with `TZ=UTC` the old and new code behave identically, so the tests have no power against #9129. The suite would need to launch the browser with `TZ` set (for example Chrome with a `TZ` environment override) for at least a UTC+ and a UTC− zone.
- `max_date=datetime.utcnow()` is passed as a `datetime`, which serializes with a time of day. It also makes the 16th unclickable if the suite is ever run in the first half of a month before the 16th (Sep 2019 is fixed, but `utcnow()` is not). The test is non-hermetic and should use a fixed `max_date`.
- `test_js_on_change_executes` and `test_server_on_change_round_trip` duplicate the same `DatePicker(...)` construction three times. A small fixture or helper would remove that. This is a minor nit.
- The file header carries the stale `2012 - 2017` copyright and `from __future__` boilerplate. That matches the sibling files, so it is not worth a finding.
- A pure `_unlocal_date` / `_to_local_date` function is unit-testable in bokehjs under `TZ=...` without Selenium. No bokehjs unit test was added.

**Remedy:** extract the normalization into a pure function, unit-test it with the encodings × zones matrix above, and add one integration assertion on the initial displayed value (`el.get_attribute('value') == 'Fri Sep 20 2019'`).

## Process note

The PR body leaves "tests added / passed" and "release document entry" unchecked. Both checkboxes in `.github/PULL_REQUEST_TEMPLATE.md` apply to a user-visible bug fix.

## Verification status

- Finding 1: **verified by execution** of the JS logic in node under four `TZ` values. The Python serialization path was confirmed by reading `serialization.py`. The browser render was not executed (unavailable).
- Finding 2: verified by reading.
- Finding 3: verified by reading. I did not run the Selenium suite (unavailable).
