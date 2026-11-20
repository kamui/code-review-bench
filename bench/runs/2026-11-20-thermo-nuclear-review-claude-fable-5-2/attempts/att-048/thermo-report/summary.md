# Thermo-nuclear review — django/django#16631

Range reviewed: `9b224579875e30203d079cc2fee83b116d98eb78..2396933ca99c6bfb53bda9e53968760316646e01` ("Fixed #34384 -- Fixed session validation when rotation secret keys.").

The change does what it sets out to do for the plain case: a session whose auth hash was made with a key now in `SECRET_KEY_FALLBACKS` is accepted and re-stamped with the current key. There are three actionable findings, two Medium and one Low. None is a security vulnerability; all are functional breaks or gaps introduced by the new code path in `get_user()`.

What was run: `auth_tests.test_basic`, `auth_tests.test_middleware` and `auth_tests.test_views` pass on the head (130 tests, OK). Findings 1 and 2 were reproduced on the head with scratch tests kept outside the clone and run through `tests/runtests.py`; the base behaviour they are compared against was read from the diff, not run. Finding 3 is from reading the code only.

## Findings

### 1. [Medium] `get_user()` now raises `AttributeError` for user objects that implement `get_session_auth_hash()` without inheriting `AbstractBaseUser`

**File:** `django/contrib/auth/__init__.py`, lines 200–221 (the call is on line 215).

`get_user()` still guards session verification with `hasattr(user, "get_session_auth_hash")` (line 200), but on a hash mismatch it now calls `user.get_session_auth_fallback_hash()` unconditionally (line 215). That method exists only on `AbstractBaseUser` (`django/contrib/auth/base_user.py`, lines 141–143). A user class that supplies its own `get_session_auth_hash()` without inheriting `AbstractBaseUser` is a documented, supported setup: `docs/topics/auth/default.txt` lines 920–923 say session verification applies if the user model "inherits from `AbstractBaseUser` or implements its own `get_session_auth_hash()` method". Any object an authentication backend returns from `get_user()` is treated the same way.

For such a user, every session whose stored hash is non-empty and does not match the current one used to be flushed and the request continued as anonymous. Now `get_user()` raises `AttributeError: ... object has no attribute 'get_session_auth_fallback_hash'` before reaching `request.session.flush()`. I reproduced this on the head with a backend returning a plain object that defines only `get_session_auth_hash()` and a session carrying a stale hash.

The failure does not depend on `SECRET_KEY_FALLBACKS` being set. It triggers in the ordinary case this feature exists for: the user changes their password (or the key is rotated), and each of their other sessions now gets a 500 on every request that touches `request.user`, instead of being logged out. Because the session is never flushed, the 500 repeats until the cookie expires or is cleared by hand.

A guard such as `hasattr(user, "get_session_auth_fallback_hash")` before the fallback check, falling through to the flush branch otherwise, restores the old behaviour for these users.

### 2. [Medium] Rotating the session key inside `get_user()` logs users out on parallel requests and on 5xx responses

**File:** `django/contrib/auth/__init__.py`, lines 217–218.

When a fallback hash matches, `get_user()` calls `request.session.cycle_key()` and then updates the hash in memory. `cycle_key()` (`django/contrib/sessions/backends/base.py`) immediately creates a new session row and deletes the old key from the store. The new key only reaches the browser if `SessionMiddleware.process_response` later saves the session and sets the cookie. Until now `get_user()` only ever read the session or flushed an invalid one; this makes an ordinary authenticated request destroy the session key the client is still holding. Two consequences follow, both in the window right after a key rotation, which is exactly when the fix is supposed to keep users logged in.

Parallel requests. A browser that sends request B with the old cookie before it has received the response to request A (a page firing several XHRs, or page plus API calls) hits a deleted key once A has run `cycle_key()`. B loads an empty session, so `request.user` is anonymous and B is redirected to login or rejected. B's response then either deletes the session cookie (`django/contrib/sessions/middleware.py` lines 36–43, when the session stays empty) or sets a cookie for a fresh anonymous session (when the view writes to the session). If B's response is processed after A's, the browser ends up logged out. I reproduced the server side of this on the head with the database backend: after A was served as `testuser`, a second client sending the same old cookie was served as anonymous and received a new `sessionid`. The empty-session cookie-deletion branch is from reading the middleware, not from a run. The signed-cookie backend is not affected, since nothing is deleted server-side.

5xx responses. `process_response` skips both `session.save()` and `set_cookie()` when the status is 500 or above (`middleware.py` lines 55–77). If the first request after rotation ends in a 5xx, the old key is already deleted, the new key is never sent, and the client is left with a dead cookie. I reproduced this on the head with a view that touches `request.user` and returns a 500: no `Set-Cookie` was sent, the old session row was gone, and the next request with the unchanged cookie was anonymous with its session data lost. An orphaned row under the new key is left behind.

The same two hazards exist for `login()` and `update_session_auth_hash()`, but those run on explicit, single form posts. Here the rotation runs on whatever request each logged-in user happens to make first after a deploy, for every user at once.

Nothing about the user's privileges changes at this point, so the key rotation is not needed for session-fixation reasons. Updating `request.session[HASH_SESSION_KEY]` in place, without `cycle_key()`, avoids both failures: concurrent requests all still find the session, and a skipped save just means the hash is upgraded on a later request.

### 3. [Low] The fallback hash ignores a model's overridden `get_session_auth_hash()`, so the fix silently does not apply to those models

**File:** `django/contrib/auth/base_user.py`, lines 141–152; `docs/topics/auth/customizing.txt`, lines 725–730.

`get_session_auth_fallback_hash()` is built on the new private `_get_session_auth_hash()`, which always computes the default HMAC of `self.password`. A model that inherits `AbstractBaseUser` and overrides `get_session_auth_hash()` (for example to mix in another field while still using `salted_hmac` with `SECRET_KEY`) stores its own hash in the session, so the default-formula fallback hashes can never match it. For these projects key rotation still logs every user out, the bug this PR fixes, with no error and no hint.

The new documentation describes the method only as "Yields the HMAC of the password field using `SECRET_KEY_FALLBACKS`" and does not say that a project overriding `get_session_auth_hash()` must override `get_session_auth_fallback_hash()` to match. This is a gap in the fix rather than a regression: these models were logged out on rotation before the change too. A sentence in the docs entry telling overriders to override both methods would close it.

## Checked and not reported

- `login()` (`django/contrib/auth/__init__.py` lines 106–116) still compares only against the current-key hash and flushes on mismatch. It is unchanged code, and in the usual flow `request.user` has already been evaluated, which upgrades the hash first.
- `session_auth_hash` on line 218 is only assigned in the `else` branch on line 205, but line 218 is reachable only when `session_hash` is truthy, so it is always bound.
- Accepting fallback-key hashes does not widen the attack surface beyond what `SECRET_KEY_FALLBACKS` already grants for signed cookies and password-reset tokens.
- `request.auser()` goes through the same `auth.get_user()` via `sync_to_async`, so it gets the same behaviour, including findings 1 and 2.
- The docs additions (the role with a backslash line continuation in `docs/ref/contrib/auth.txt`, the `versionchanged`/`versionadded` notes, and the 4.1.8 release note) are consistent with the code.
- No developer-experience breaks (settings, environment, scripts) and no feature-gate leaks: the change adds no settings and reads only the existing `SECRET_KEY_FALLBACKS`.
