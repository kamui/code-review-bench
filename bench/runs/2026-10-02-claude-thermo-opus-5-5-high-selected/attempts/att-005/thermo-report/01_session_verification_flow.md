# 01 — Session verification flow (`django/contrib/auth/__init__.py`)

Scope: the `get_user()` hunk at `django/contrib/auth/__init__.py:200-221`, and
its relationship to the untouched sibling check in `login()` at
`django/contrib/auth/__init__.py:106-116`.

## Measurements

Commands, all run from the clone root, read-only:

- `git diff main...review-head` — the full change (6 files, +69 / −7).
- `wc -l django/contrib/auth/__init__.py django/contrib/auth/base_user.py tests/auth_tests/test_basic.py`
  → 244 / 167 / 164 lines. No file is anywhere near the 1k-line threshold, so
  the file-size rule does not apply to this PR.
- `PYTHONPATH=<clone> <cache>/venv/bin/python tests/runtests.py auth_tests.test_basic --settings=test_sqlite`
  → `Ran 13 tests in 0.305s — OK`. The PR's own test passes at the head SHA.

Shape of the verification block before and after:

| | base (`main`) | head (`review-head`) |
| --- | --- | --- |
| lines in the block | 8 | 22 |
| max nesting below `def get_user` | 4 | 6 |
| `session_hash` truthiness tests | 1 | 2 (lines 202 and 213) |
| names bound on only one branch | 0 | 1 (`session_auth_hash`, line 205) |
| session side effects | `flush()` | `flush()`, `cycle_key()`, item assignment |

## Finding A — unguarded call to `get_session_auth_fallback_hash()`

Evidence. The whole block is gated on duck typing:

```python
if hasattr(user, "get_session_auth_hash"):        # line 200
    ...
    if not session_hash_verified:                 # line 209
        if session_hash and any(                  # line 213
            constant_time_compare(session_hash, fallback_auth_hash)
            for fallback_auth_hash in user.get_session_auth_fallback_hash()  # line 215
        ):
```

The gate checks for one method and the body calls a different, brand-new one.
The duck-typed shape is not hypothetical: `docs/topics/auth/default.txt:919-922`
says session invalidation applies "if your `AUTH_USER_MODEL` inherits from
`AbstractBaseUser` **or implements its own `get_session_auth_hash()` method**".
A user class of the second kind has no `get_session_auth_fallback_hash`.

Failure path. Such a user changes their password (or is logged in on a second
device when the password changes). The stale session carries a non-empty
`HASH_SESSION_KEY` that no longer matches. On `main` the session is flushed and
the request proceeds anonymously — that is the documented "log out all of their
sessions" behaviour. On the head, line 213 passes the `session_hash` test and
line 215 raises `AttributeError`. The session is never flushed, so the same
cookie raises again on every subsequent request. The feature the hash exists
for (invalidate on password change) turns into a persistent 500 for that
browser. `SECRET_KEY_FALLBACKS` does not have to be set for this to happen.

Verification status: reasoned from source; not executed. The allowance permits
only `tests/runtests.py` selections against an unmodified clone, and no
existing test uses a user class that defines `get_session_auth_hash` without
inheriting `AbstractBaseUser` (`grep -rn get_session_auth_hash tests` returns
no hits), so there is nothing to select that would exercise it.

Remedy. Do not make the fallback method part of the gate's implicit contract.
Either feature-detect it the same way the primary method is detected, or — the
cleaner option — have the helper proposed below treat "no fallback method" as
"no fallback hashes":

```python
fallback_hashes = getattr(user, "get_session_auth_fallback_hash", lambda: ())()
```

and add a test with a minimal non-`AbstractBaseUser` user object.

## Finding B — three outcomes encoded as a flag plus nested re-tests

Evidence (lines 201-221 at head):

```python
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

There are exactly three outcomes — matches the current key, matches a fallback
key, matches nothing — but the code spells them as a boolean flag, an
`if/else` that exists only to compute the flag, a second `if` on the flag, and
a third `if` that re-tests `session_hash`. The second `session_hash` test on
line 213 is not there for the fallback logic at all: it is what keeps
`session_auth_hash` on line 218 from being read while unbound, because that
name is assigned only in the `else:` on line 205. Correctness therefore hangs
on a reader noticing that two `session_hash` tests thirteen lines apart must
stay in sync. This is the "narrow edge-case handling implemented in the middle
of an already busy function" pattern: `get_user()` is now six levels deep and
mixes backend lookup, hash policy and session mutation.

Worked code-judo proposal. Pull the policy into one small function that
answers a single question and owns the upgrade, so `get_user()` goes back to
one condition and one consequence:

```python
def _verify_session_auth_hash(request, user):
    """
    Return True if the session's auth hash matches the user's hash under the
    current secret or any fallback secret. A fallback match is upgraded to the
    current secret.
    """
    session_hash = request.session.get(HASH_SESSION_KEY)
    if not session_hash:
        return False
    session_auth_hash = user.get_session_auth_hash()
    if constant_time_compare(session_hash, session_auth_hash):
        return True
    fallback_hashes = getattr(user, "get_session_auth_fallback_hash", lambda: ())()
    if any(constant_time_compare(session_hash, h) for h in fallback_hashes):
        request.session[HASH_SESSION_KEY] = session_auth_hash
        return True
    return False
```

```python
# get_user()
if hasattr(user, "get_session_auth_hash") and not _verify_session_auth_hash(
    request, user
):
    request.session.flush()
    user = None
```

What disappears: the `session_hash_verified` flag, the duplicated
`session_hash` test, the conditionally bound name, two nesting levels inside
`get_user()`, and the latent `AttributeError` from Finding A. What stays the
same: the short-circuit that avoids computing an HMAC when the session has no
hash, the constant-time comparisons, and the flush-on-failure behaviour.
Whether the upgrade branch should also call `cycle_key()` is Finding C.

Verification status: read from source; behaviour-equivalence of the proposal
was checked by hand against the four input cases (no hash / current match /
fallback match / no match), not executed.

## Finding C — `cycle_key()` on the read path

Evidence. Line 217 calls `request.session.cycle_key()` inside `get_user()`.
`SessionBase.cycle_key()` (`django/contrib/sessions/backends/base.py:298-307`)
creates a new session row and then **deletes the old key**. `get_user()` is
the lazy getter behind `request.user`; until this PR its only side effect was
flushing a session it had already decided was invalid.

Why it matters. A key rotation is a deploy-time event that hits every live
session at once, and a browser routinely sends several requests in parallel
with the same cookie. Request A upgrades the hash, gets a new session key and
deletes the old row. Request B, carrying the old cookie, then calls
`SessionStore.load()` (`django/contrib/sessions/backends/db.py:41-43`), finds
no row and gets `{}`; `get_user()` returns `AnonymousUser`; and
`SessionMiddleware.process_response`
(`django/contrib/sessions/middleware.py:31-37`) sees a cookie on the request
and an empty session and emits `delete_cookie`. If B's response lands after
A's, the browser drops the freshly issued cookie and the user is logged out —
the outcome the ticket is trying to prevent. The fix does not need the cycle:
the session key is random and not derived from `SECRET_KEY`, and rewriting
`HASH_SESSION_KEY` in place is already enough to make the session survive
removal of the fallback (the PR's second `override_settings` block proves
exactly that property). `update_session_auth_hash()` cycles because a password
just changed; nothing comparable happened here.

Verification status: reasoned from source across the three files cited; the
race was not reproduced (it needs concurrent requests against a live session
store, which is outside the test-selection allowance).

Remedy. Drop `cycle_key()` from the upgrade branch and the matching
`assertNotEqual(request.session.session_key, prev_session_key)` assertion, or,
if cycling is a deliberate security decision, say why in the comment and
accept the concurrency cost explicitly. As written, the only comment in the
block describes the `any()` loop and says nothing about why a getter rotates
session identity.

## Finding D — the same check exists in `login()` and was not updated

Evidence. `login()` at `django/contrib/auth/__init__.py:106-116`:

```python
if SESSION_KEY in request.session:
    if _get_user_session_key(request) != user.pk or (
        session_auth_hash
        and not constant_time_compare(
            request.session.get(HASH_SESSION_KEY, ""), session_auth_hash
        )
    ):
        request.session.flush()
```

This is the same "does the stored hash belong to this user" decision, compared
against the current-key hash only. After this PR the two call sites disagree:
`get_user()` accepts a fallback-key hash and upgrades it, `login()` treats the
same session as foreign and flushes it. `request.user` is lazy, so a request
that reaches `login()` for the already-logged-in user without first touching
`request.user` loses its session data during the rotation window. The impact
is smaller than the ticket's (the user ends up authenticated, but anything
else stored in the session is gone), but the root cause is structural: the
policy is written out inline twice and the PR patched one copy.

Verification status: read from source; not executed.

Remedy. Once the comparison lives in a helper (Finding B), `login()` should
call the same helper so "which stored hashes are acceptable" has one owner.
