# 01 — Session verification flow (`django/contrib/auth/__init__.py`)

Covers F1, F2, F3 and F4 from `summary.md`.

## The code under review

`django/contrib/auth/__init__.py:198-221` on `review-head`:

```python
            user = backend.get_user(user_id)
            # Verify the session
            if hasattr(user, "get_session_auth_hash"):
                session_hash = request.session.get(HASH_SESSION_KEY)
                if not session_hash:
                    session_hash_verified = False
                else:
                    session_auth_hash = user.get_session_auth_hash()
                    session_hash_verified = constant_time_compare(
                        session_hash, session_auth_hash
                    )
                if not session_hash_verified:
                    # If the current secret does not verify the session, try
                    # with the fallback secrets and stop when a matching one is
                    # found.
                    if session_hash and any(
                        constant_time_compare(session_hash, fallback_auth_hash)
                        for fallback_auth_hash in user.get_session_auth_fallback_hash()
                    ):
                        request.session.cycle_key()
                        request.session[HASH_SESSION_KEY] = session_auth_hash
                    else:
                        request.session.flush()
                        user = None
```

On `main` the same block was:

```python
            if hasattr(user, "get_session_auth_hash"):
                session_hash = request.session.get(HASH_SESSION_KEY)
                session_hash_verified = session_hash and constant_time_compare(
                    session_hash, user.get_session_auth_hash()
                )
                if not session_hash_verified:
                    request.session.flush()
                    user = None
```

## Measurements

| Measure | `main` | `review-head` |
| --- | --- | --- |
| Lines in the verification block | 8 | 22 |
| Deepest indentation inside `get_user()` | 5 levels | 6 levels (lines 214-218) |
| Longest line in the block | 79 columns | 87 columns (line 215) |
| Tests of `session_hash` truthiness | 1 | 2 (lines 202 and 213) |
| Locals bound on only one branch | 0 | 1 (`session_auth_hash`, line 205, read at line 218) |
| File length | 230 | 244 |

Commands:

```text
git diff main...review-head -- django/contrib/auth/__init__.py
wc -l django/contrib/auth/__init__.py                       -> 244
awk 'FNR>=196 && FNR<=222 {print FNR": "length}' django/contrib/auth/__init__.py | sort -t: -k2 -n | tail -2
                                                             -> 214: 79, 215: 87
```

## How the probes were run

The PR's own tests, from the clone root:

```text
PYTHONPATH=<clone> <cache>/venv/bin/python tests/runtests.py auth_tests.test_basic --settings=test_sqlite
Ran 13 tests in 0.040s — OK
```

The probes are four test methods in a scratch module at
`<work>/scratch/thermo_scratch/tests.py`, outside the clone, run once with the
target's runner:

```text
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=<clone>:<work>/scratch \
  <cache>/venv/bin/python tests/runtests.py thermo_scratch.tests --settings=test_sqlite
Ran 4 tests in 0.016s — OK
```

Each probe prints what it observed instead of asserting, so "OK" only means
the probes ran. `git status --short` in the clone was empty afterwards.

Raw output:

```text
P1 RESULT: AttributeError: 'DuckUser' object has no attribute 'get_session_auth_fallback_hash'
P2 RESULT: after rotation -> AnonymousUser
P3 RESULT: no rotation, cart = kept
P3 RESULT: after rotation, cart = None
P4 RESULT: request A -> User | key rotated: True | old row exists: False | request B (old cookie) -> AnonymousUser | B session empty (middleware deletes cookie): True
```

Base-branch behaviour was not re-executed. Where this file says what `main`
does, that is read from the removed lines of the diff.

## F1 — guard and call disagree

**Status: reproduced on `review-head` (P1). Baseline by reading.**

The guard at line 200 is duck-typed on `get_session_auth_hash`. The new call at
line 215 assumes `get_session_auth_fallback_hash` exists whenever the first one
does. That is only true for subclasses of `AbstractBaseUser`.

The supported alternative is written down in
`docs/topics/auth/default.txt:920-922`:

> If your `AUTH_USER_MODEL` inherits from `AbstractBaseUser` or implements its
> own `get_session_auth_hash()` method, authenticated sessions will include the
> hash returned by this function.

Probe P1 sets up exactly that: a backend whose `get_user()` returns an object
with `get_session_auth_hash()` and nothing else, and a session whose stored
hash is non-empty and stale.

```python
class DuckUser:
    pk = 1
    is_authenticated = True

    def get_session_auth_hash(self):
        return "current-hash"
```

Result on `review-head`: `AttributeError`. On `main` the removed lines flush the
session and return `AnonymousUser`.

Points worth stating plainly:

- The trigger is "stored hash is non-empty and does not equal the current
  hash". That is the password-changed-in-another-session case, which is the
  reason the block exists.
- `SECRET_KEY_FALLBACKS` being empty does not help. The method is called before
  anything looks at the setting.
- The session is never flushed, so the failure repeats on every request with
  that cookie.
- The exception surfaces wherever `request.user` is first touched, since
  `get_user()` runs lazily.

Remedy: resolve the hook defensively, inside the helper proposed under F3.

```python
get_fallback_hashes = getattr(user, "get_session_auth_fallback_hash", None)
```

## F2 — `cycle_key()` inside verification

**Status: reproduced on `review-head` (P4). Baseline by reading: `main` never
calls `cycle_key()` from `get_user()`.**

`django/contrib/sessions/backends/base.py:298-307`:

```python
    def cycle_key(self):
        data = self._session
        key = self.session_key
        self.create()
        self._session_cache = data
        if key:
            self.delete(key)
```

`create()` saves a new row at once and `delete(key)` removes the old row at
once. Both happen at the moment `request.user` is first evaluated, not when the
response is written.

Probe P4, under `SECRET_KEY="newsecret"` with the old key in
`SECRET_KEY_FALLBACKS`:

1. Request A loads the session by the old key and calls `get_user()`. It gets
   the `User`, the session key changes, the old row no longer exists.
2. Request B loads the session by the same old key and calls `get_user()`. It
   gets `AnonymousUser`, and `b.session.is_empty()` is `True`.

Why step 2 matters beyond one anonymous response:
`django/contrib/sessions/middleware.py:34-43` deletes the session cookie when a
request arrives with a session cookie and ends with an empty session.

```python
        if settings.SESSION_COOKIE_NAME in request.COOKIES and empty:
            response.delete_cookie(
                settings.SESSION_COOKIE_NAME,
```

If B's response reaches the browser after A's, the cookie A just issued is
removed and the user is logged out. A request that loaded the session before
A's `cycle_key()` and then saves a modified session afterwards instead hits
`UpdateError`, which the middleware turns into `SessionInterrupted`
(`middleware.py:55-66`).

Not established by the probe: how often real traffic hits this window. The
probe shows the ordering is sufficient, not how frequent it is. The window
opens for each logged-in user on their first authenticated request after a
rotation deploy, and stays open until the browser has stored the new cookie.

Comparison with existing callers of `cycle_key()`:

| Caller | Trigger | Frequency |
| --- | --- | --- |
| `login()` (line 118) | explicit login of an anonymous session | once per login |
| `update_session_auth_hash()` (line 242) | explicit password change | rare |
| `get_user()` (line 217, new) | any request that evaluates `request.user` | once per user per rotation, on an arbitrary request |

Remedy: replace lines 217-218 with the single assignment. It is idempotent, so
concurrent requests converge on the same stored value, and the session row
never disappears. If a security reason for cycling exists, it should be written
next to the call (see Q1 in the summary).

## F3 — the tangle, and the move that removes it

**Status: structural, verified by reading.**

What a reader must reconstruct to trust lines 200-221:

1. `session_hash_verified` is `False` either because there was no hash or
   because the current-key comparison failed.
2. Line 213 re-tests `session_hash` to tell those two cases apart again.
3. That re-test is also the only thing guaranteeing `session_auth_hash` is
   bound at line 218, because it is assigned only in the `else` at line 205.

The flag collapses two states into one and the next statement has to pull them
apart. Nothing in the code says that the second `session_hash` test is
load-bearing for name binding.

### Worked proposal

One helper, early returns, no flag:

```python
def _verify_session_auth_hash(request, user):
    """
    Return whether the hash stored in the session matches the user's session
    auth hash. A hash that only matches under one of SECRET_KEY_FALLBACKS is
    replaced with the hash for the current SECRET_KEY.
    """
    session_hash = request.session.get(HASH_SESSION_KEY)
    if not session_hash:
        return False
    session_auth_hash = user.get_session_auth_hash()
    if constant_time_compare(session_hash, session_auth_hash):
        return True
    get_fallback_hashes = getattr(user, "get_session_auth_fallback_hash", None)
    if get_fallback_hashes is None or not any(
        constant_time_compare(session_hash, fallback_auth_hash)
        for fallback_auth_hash in get_fallback_hashes()
    ):
        return False
    request.session[HASH_SESSION_KEY] = session_auth_hash
    return True
```

`get_user()` returns to its pre-PR shape:

```python
            user = backend.get_user(user_id)
            # Verify the session
            if hasattr(
                user, "get_session_auth_hash"
            ) and not _verify_session_auth_hash(request, user):
                request.session.flush()
                user = None
```

What this buys, item by item:

| Problem in the patch | In the proposal |
| --- | --- |
| `session_hash_verified` flag set then re-tested | gone, replaced by `return` |
| `session_hash` tested twice | tested once |
| `session_auth_hash` bound on one branch only | bound unconditionally before any read |
| Six levels of nesting, 87-column line | helper peaks at two levels |
| Missing hook raises `AttributeError` (F1) | `getattr` with `None` default |
| `cycle_key()` in a read path (F2) | plain assignment |
| `login()` unaware of fallbacks (F4) | calls the same helper, see below |

Behaviour preserved: missing hash is rejected, current-key match is accepted
with no session write, fallback match re-signs, anything else is rejected and
the caller flushes. The only intended differences from the patch are F1 and F2.

If the maintainers want to keep `cycle_key()`, it slots into the helper as one
line before the assignment and everything else above still holds.

## F4 — `login()` kept the old rule

**Status: reproduced on `review-head` (P3). The lines are untouched by the PR,
so `main` behaves the same here; the finding is that the PR changed the rule in
one of two places.**

`django/contrib/auth/__init__.py:101-116`:

```python
    if hasattr(user, "get_session_auth_hash"):
        session_auth_hash = user.get_session_auth_hash()

    if SESSION_KEY in request.session:
        if _get_user_session_key(request) != user.pk or (
            session_auth_hash
            and not constant_time_compare(
                request.session.get(HASH_SESSION_KEY, ""), session_auth_hash
            )
        ):
            # To avoid reusing another user's session, create a new, empty
            # session if the existing session corresponds to a different
            # authenticated user.
            request.session.flush()
```

Probe P3 logs a user in, stores `cart` in the session, and calls `login()`
again for the same user:

- without rotation, `cart` is still there;
- with a new `SECRET_KEY` and the old one in `SECRET_KEY_FALLBACKS`, `cart` is
  gone.

So after rotation `get_user()` says the session is valid and `login()` says it
belongs to someone else. Which one a request meets first depends on whether
anything evaluated `request.user` before `login()` ran. `LoginView.dispatch`
only evaluates it when `redirect_authenticated_user` is set.

Impact is small: the user ends up logged in either way, and only session data
is lost. It is listed because it is the visible symptom of the duplication.

With the helper from F3:

```python
    if SESSION_KEY in request.session:
        if _get_user_session_key(request) != user.pk or (
            session_auth_hash and not _verify_session_auth_hash(request, user)
        ):
            request.session.flush()
```

The helper recomputes `user.get_session_auth_hash()` once more than the inline
version. If that matters, pass the already computed hash in as a third
argument; the structure does not change.
