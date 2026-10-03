# 01 — `get_user()` session verification (F1, F3, F4, F6)

Subsystem: `django/contrib/auth/__init__.py`, with supporting reads of `django/contrib/sessions/backends/base.py`, `django/contrib/sessions/backends/db.py`, `django/contrib/sessions/backends/signed_cookies.py`, `django/contrib/sessions/middleware.py`, `django/contrib/auth/tokens.py` and `django/core/signing.py`.

## What the diff did

Before (base `9b22457987`):

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

After (head `2396933ca9`, lines 200–221):

```python
if hasattr(user, "get_session_auth_hash"):                       # 200
    session_hash = request.session.get(HASH_SESSION_KEY)         # 201
    if not session_hash:                                         # 202
        session_hash_verified = False
    else:
        session_auth_hash = user.get_session_auth_hash()         # 205
        session_hash_verified = constant_time_compare(
            session_hash, session_auth_hash
        )
    if not session_hash_verified:                                # 209
        # If the current secret does not verify the session, try
        # with the fallback secrets and stop when a matching one is
        # found.
        if session_hash and any(                                 # 213
            constant_time_compare(session_hash, fallback_auth_hash)
            for fallback_auth_hash in user.get_session_auth_fallback_hash()   # 215
        ):
            request.session.cycle_key()                          # 217
            request.session[HASH_SESSION_KEY] = session_auth_hash  # 218
        else:
            request.session.flush()                              # 220
            user = None
```

Measurements: `wc -l` gives 244 lines for `django/contrib/auth/__init__.py` at head, so there is no file-size concern. The block grew from 8 lines and one branch to 22 lines and four branch arms. Deepest indentation inside `get_user()` is now six levels (function, `else`, `if backend_path`, `if hasattr`, `if not session_hash_verified`, `if session_hash and any`).

## F1 — unguarded call to `get_session_auth_fallback_hash()`

### Evidence

The guard on line 200 is `hasattr(user, "get_session_auth_hash")`. It is not `isinstance(user, AbstractBaseUser)`. The same duck-typed guard appears in `login()` (line 103) and `update_session_auth_hash()` (line 243). The reason is documented at `docs/topics/auth/default.txt` lines 919–922:

> If your `AUTH_USER_MODEL` inherits from `AbstractBaseUser` or implements its own `get_session_auth_hash()` method, authenticated sessions will include the hash returned by this function.

So a user model that implements its own method without inheriting from `AbstractBaseUser` is an explicitly supported configuration.

Line 215 evaluates `user.get_session_auth_fallback_hash()` as the outermost iterable of the generator expression passed to `any()`. Python evaluates the outermost iterable eagerly when the generator expression is created, so the attribute lookup happens as soon as line 213's `session_hash and ...` reaches its right-hand side, that is, whenever `session_hash` is truthy and the current-key comparison failed.

### Failure scenario

1. Project has a user model with `get_session_auth_hash()` defined but not inherited from `AbstractBaseUser`.
2. A user has two logged-in sessions and changes their password in one of them. The other session's stored hash no longer matches.
3. Next request on the stale session: line 201 reads a non-empty hash, line 205–208 computes a mismatch, line 213 evaluates `session_hash` (truthy) and then line 215 raises `AttributeError: '...' object has no attribute 'get_session_auth_fallback_hash'`.
4. Because the exception propagates before line 220, the session is never flushed. Every subsequent request with that cookie raises again until the session expires.

Before the PR the same request flushed the session and returned `AnonymousUser`. Note that `SECRET_KEY_FALLBACKS` does not need to be configured for this to happen.

### Verification status

Code trace only. Not executed: reproducing it needs a custom user model that the existing test apps do not provide, and the clone must not be modified.

### Remedy

Make the second method optional at the call site. This falls out of the helper in F4 below.

## F3 — `cycle_key()` on the read path

### Evidence

`SessionBase.cycle_key()` at `django/contrib/sessions/backends/base.py` lines 298–307:

```python
def cycle_key(self):
    data = self._session
    key = self.session_key
    self.create()
    self._session_cache = data
    if key:
        self.delete(key)
```

The old key is deleted in the same call. For the DB backend, a later `load()` of the old key goes through `_get_session_from_db()`, finds no row, sets `self._session_key = None` and returns `{}` (`django/contrib/sessions/backends/db.py` lines 41–43). `SessionBase.is_empty()` (base.py lines 139–144) is then true.

`SessionMiddleware.process_response()` at `django/contrib/sessions/middleware.py` lines 36–43:

```python
if settings.SESSION_COOKIE_NAME in request.COOKIES and empty:
    response.delete_cookie(settings.SESSION_COOKIE_NAME, ...)
```

`get_user()` is reached through `request.user` (`django/contrib/auth/middleware.py` line 13), which is a lazy object evaluated on first access in any view that checks authentication.

### Failure scenario

After the operator rotates `SECRET_KEY` and moves the old key into `SECRET_KEY_FALLBACKS`:

1. Browser loads a page that issues requests A and B in parallel, both with session cookie `K_old`.
2. Request A reaches `get_user()`, matches a fallback, calls `cycle_key()`: creates `K_new`, deletes `K_old`. A's response sets cookie `K_new`.
3. Request B has not yet loaded its session (sessions load lazily on first access). It now loads `K_old`, finds nothing, gets an empty session. `_get_user_session_key()` raises `KeyError`, B is served as `AnonymousUser` — a redirect to login or a 403 for an XHR.
4. B's response carries `delete_cookie` for the session cookie. If B's response is processed by the browser after A's, the `K_new` cookie is removed and the user is fully logged out.

Even when response ordering is favourable, request B was answered as anonymous. The whole purpose of ticket #34384 is that users are not logged out by a key rotation.

The existing callers of `cycle_key()` in this module — `login()` (line 117) and `update_session_auth_hash()` (line 242) — are deliberate, user-initiated, one-per-action requests. `get_user()` is not: it runs on every authenticated request, so the first wave of requests from each client after a rotation all take this path.

### Is the rotation needed?

For `db`, `cache`, `cached_db` and `file` backends the session key is a random string from `_get_new_session_key()`; it has no relationship to `SECRET_KEY`. Rotating it after a secret-key change does not invalidate anything an attacker holding the old secret could forge. For `signed_cookies`, `cycle_key()` is overridden to `self.save()` (`signed_cookies.py` lines 60–65), so nothing is rotated there either. Line 218's `request.session[HASH_SESSION_KEY] = ...` sets `modified = True`, which is enough for the middleware to save the upgraded hash at the end of the request.

It also turns a getter into a write-heavy operation: one `INSERT` and one `DELETE` on the session table from inside `get_user()`, in addition to the save at response time.

### Verification status

Each step of the mechanism was read in the code at the pinned head. The race was not reproduced; doing so needs a concurrent harness that is outside the execution allowance. Confidence in the mechanism is high; whether the project considers the rotation worth that cost is a judgment call for the author, which is why the remedy is phrased as "drop it or justify it".

### Remedy

Delete line 217 and the `assertNotEqual(request.session.session_key, prev_session_key)` assertion at `tests/auth_tests/test_basic.py` line 158. If the rotation is intentional, the comment above it should say what it protects against and `docs/ref/contrib/auth.txt` should mention that the session key changes.

## F4 — flag-and-recheck control flow; worked code-judo proposal

### Evidence

Three things a reader has to hold at once in the new block:

- `session_hash` truthiness is tested at line 202 and again at line 213.
- `session_hash_verified` exists only to carry the result of lines 202–208 to line 209.
- `session_auth_hash` is bound at line 205 only in the `else` arm, and read at line 218. That read is safe only because line 213 re-tests `session_hash`; if someone "simplifies" the duplicate test away, line 218 raises `UnboundLocalError` for sessions with no stored hash.

The codebase already has the canonical shape for "verify against the current secret, then the fallbacks":

`django/contrib/auth/tokens.py` line 69:

```python
for secret in [self.secret, *self.secret_fallbacks]:
    if constant_time_compare(self._make_token_with_timestamp(user, ts, secret), token):
        break
else:
    return False
```

`django/core/signing.py` line 235:

```python
for key in [self.key, *self.fallback_keys]:
    if constant_time_compare(sig, self.signature(value, key)):
        return value
```

### Worked proposal

Pull the verification out of `get_user()` into a helper with early returns. This is behaviour-preserving for stock user models, apart from the two deliberate changes called out in comments.

```python
def _verify_session_auth_hash(request, user):
    """
    Return True if the hash stored in the session matches the user's session
    auth hash for the current secret or for one of SECRET_KEY_FALLBACKS. In
    the latter case, upgrade the stored hash to the current secret.
    """
    session_hash = request.session.get(HASH_SESSION_KEY)
    if not session_hash:
        return False
    session_auth_hash = user.get_session_auth_hash()
    if constant_time_compare(session_hash, session_auth_hash):
        return True
    # User models that only implement get_session_auth_hash() have no
    # fallbacks. (F1)
    get_fallback_hashes = getattr(user, "get_session_auth_fallback_hash", None)
    if get_fallback_hashes is None:
        return False
    if any(
        constant_time_compare(session_hash, fallback_auth_hash)
        for fallback_auth_hash in get_fallback_hashes()
    ):
        # No cycle_key(): the session key is independent of SECRET_KEY. (F3)
        request.session[HASH_SESSION_KEY] = session_auth_hash
        return True
    return False
```

`get_user()` returns to its pre-PR size and shape:

```python
            user = backend.get_user(user_id)
            # Verify the session
            if hasattr(user, "get_session_auth_hash") and not (
                _verify_session_auth_hash(request, user)
            ):
                request.session.flush()
                user = None
```

What this deletes: the `session_hash_verified` flag, the second `session_hash` test, the conditional binding of `session_auth_hash`, and two levels of nesting. What it adds: one named concept, "the session auth hash verifies", which `login()` can also use (F6).

## F6 — `login()` has the same comparison and was not updated

### Evidence

`django/contrib/auth/__init__.py` lines 106–115, unchanged by the PR:

```python
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

`session_auth_hash` here is `user.get_session_auth_hash()` under the current secret (line 104). A session whose stored hash was produced under a fallback secret fails this comparison.

### Failure scenario

After a rotation, a logged-in user with a not-yet-upgraded session submits the login form again as the same user. `LoginView.dispatch()` only touches `request.user` when `redirect_authenticated_user` is true, which is not the default, so `get_user()` may not have run and upgraded the hash before `form_valid()` calls `login()`. `login()` sees `SESSION_KEY` present, same `user.pk`, mismatching hash, and flushes. Session data that same-user re-login preserved before the rotation is lost. The user is still logged in afterwards.

### Verification status

Code trace only. Lower severity than F1–F3 and in pre-existing code, but it is the same comparison the PR set out to fix and it is the natural second caller of the helper above:

```python
if SESSION_KEY in request.session:
    if _get_user_session_key(request) != user.pk or (
        session_auth_hash and not _verify_session_auth_hash(request, user)
    ):
        request.session.flush()
```

One behavioural nuance to check when making that change: `login()` today treats a missing stored hash as a mismatch via the `""` default, and the helper returns `False` for a missing hash, so the two agree.

## Commands

```text
git diff main...review-head
sed -n 90,275p django/contrib/auth/__init__.py
wc -l django/contrib/auth/__init__.py django/contrib/auth/base_user.py tests/auth_tests/test_basic.py
sed -n 1,80p django/contrib/sessions/middleware.py
grep -n "def cycle_key" -A 14 django/contrib/sessions/backends/base.py
grep -n "def load" -A 12 django/contrib/sessions/backends/db.py
sed -n 55,80p django/contrib/auth/tokens.py
PYTHONPATH=$PWD ../clone-cache/venv/bin/python tests/runtests.py auth_tests.test_basic --settings=test_sqlite
    -> Ran 13 tests in 0.201s, OK
```
