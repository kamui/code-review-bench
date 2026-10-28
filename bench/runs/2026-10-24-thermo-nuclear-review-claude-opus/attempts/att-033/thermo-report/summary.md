# Thermo Nuclear Review — django/django#16631

Range: `9b224579875e30203d079cc2fee83b116d98eb78..2396933ca99c6bfb53bda9e53968760316646e01` ("Fixed #34384 -- Fixed session validation when rotation secret keys.")

The change makes `django.contrib.auth.get_user()` accept a session auth hash computed with any key in `SECRET_KEY_FALLBACKS`. When a fallback hash matches, it cycles the session key and rewrites the stored hash using the current `SECRET_KEY`. It also adds `AbstractBaseUser.get_session_auth_fallback_hash()` and a private `_get_session_auth_hash(secret=None)`.

Verification performed: I traced `get_user()` through `AuthenticationMiddleware` (sync `request.user` and async `request.auser()`), `SessionBase.cycle_key()`, the db session backend's `create()`/`delete()`, and `SessionMiddleware.process_response()`. I ran `auth_tests.test_basic auth_tests.test_middleware sessions_tests`: 394 tests, OK. I also ran a scratch script outside the clone that reproduces findings 1 and 2 below.

## Findings

### 1. [Medium] `get_user()` now calls `get_session_auth_fallback_hash()` without checking it exists, so user objects that implement only `get_session_auth_hash()` crash with `AttributeError` instead of being logged out

File: `django/contrib/auth/__init__.py`, lines 200–221 (the call is on line 215). Related: `django/contrib/auth/base_user.py`, lines 141–143.

Evidence: The verification block is still guarded by `if hasattr(user, "get_session_auth_hash"):`. Django keeps this duck-typed protocol on purpose, because backends can return user objects that don't inherit from `AbstractBaseUser`; `login()` and `update_session_auth_hash()` use the same guard. Inside that guard, whenever the stored hash is non-empty and doesn't match, the new code calls `user.get_session_auth_fallback_hash()` unconditionally. Only `AbstractBaseUser` defines that method. Anyone who implements `get_session_auth_hash()` on their own user class (a non-ORM user object, or a custom backend's user type) gets an `AttributeError` here.

Failure: A session's stored hash can legitimately stop matching, for example after a password change in another browser or a password reset. Before this change, `get_user()` flushed that session and returned `AnonymousUser`. Now it raises `AttributeError` before `request.session.flush()` runs. The session row keeps its auth data, so every later request that touches `request.user` or `await request.auser()` with that cookie fails with a 500 until the cookie expires or is cleared by hand. This happens whether or not `SECRET_KEY_FALLBACKS` is set, so a plain password change now locks such users into an error loop. It ships in a patch release (4.1.8). Scratch reproduction: a backend returning a `LegacyUser` that has only `get_session_auth_hash()`, with a stale session hash, made `get_user()` raise `AttributeError: 'LegacyUser' object has no attribute 'get_session_auth_fallback_hash'`, and the stored session still held `_auth_user_id`. A `hasattr`/`getattr` fallback to an empty iterable would keep the old flush-and-logout behaviour.

### 2. [Medium] Calling `cycle_key()` on the first request after rotation deletes the old session row, so concurrent requests sent with the old cookie are logged out and can delete the new cookie

File: `django/contrib/auth/__init__.py`, lines 211–218 (`request.session.cycle_key()` on line 217).

Evidence: `SessionBase.cycle_key()` (`django/contrib/sessions/backends/base.py`, lines 298–307) creates and persists a new session right away, then immediately calls `self.delete(old_key)`. The new code runs this the first time `request.user` is evaluated for every pre-rotation session, i.e. on every logged-in user's first request after a key rotation. Browsers routinely send several requests in parallel with the same cookie (page load plus XHR/fetch calls, multiple tabs, asset views that check auth). Any of those requests that loads its session after the first request deleted the old row gets an empty session, so `get_user()` returns `AnonymousUser`. Then `SessionMiddleware.process_response()` sees the old cookie with an empty session and calls `response.delete_cookie(SESSION_COOKIE_NAME)`. If that response reaches the browser after the one carrying the new cookie, the new cookie is wiped. A concurrent request that loaded the old session before the delete and then modifies the session without touching `request.user` hits `UpdateError` → `SessionInterrupted` (HTTP 400) on save. Also, if the first request ends in a 5xx response, `SessionMiddleware` skips `set_cookie`, but the old row is already deleted, so the browser keeps a dead key.

Failure: This brings back spurious logouts during secret rotation, which is the bug #34384 is meant to fix, most visibly for SPA or AJAX-heavy sites right after deploying a rotated key. Scratch reproduction with the db backend: two `SessionStore`s for the same pre-rotation key. After request A's `get_user()` succeeded via the fallback and cycled the key, request B's `get_user()` returned `AnonymousUser` and its session was empty. Re-keying gives no security benefit here, because the session key isn't derived from `SECRET_KEY`. Only rewriting `HASH_SESSION_KEY` is needed to upgrade the hash.

### 3. [Low] The fallback check ignores an overridden `get_session_auth_hash()`, so custom user models that override it still lose every session on rotation

File: `django/contrib/auth/base_user.py`, lines 135–151; `django/contrib/auth/__init__.py`, line 215.

Evidence: `get_session_auth_hash()` now delegates to the private `_get_session_auth_hash()`, and `get_session_auth_fallback_hash()` calls `_get_session_auth_hash(secret=...)` directly, using the base salt and only the password field. A subclass that overrides `get_session_auth_hash()` (for example, to add a per-user session version, which is a documented customisation point) stores hashes in its own format. The inherited fallback generator still yields base-format hashes, so they never match. The docs added in `docs/topics/auth/customizing.txt` and `docs/ref/contrib/auth.txt` don't mention that such models must also override `get_session_auth_fallback_hash()`.

Failure: For these models, rotating `SECRET_KEY` with the old key in `SECRET_KEY_FALLBACKS` still flushes every session. The fix silently doesn't apply, and each request with a stale hash also computes `len(SECRET_KEY_FALLBACKS)` extra HMACs before flushing. No security break: the fallback hashes are still keyed on secrets and the password hash, so they can't be forged without those.

## Other areas checked (no issue)

- `session_auth_hash` is only bound when `session_hash` is truthy, and the fallback branch also requires `session_hash`, so there is no `UnboundLocalError`.
- `aget_user`/`auser()` wraps the sync `get_user()` via `sync_to_async`, so the async path gets the same behaviour, including findings 1 and 2.
- `constant_time_compare` is still used per candidate. Short-circuiting in `any()` only reveals which fallback index matched, which is not exploitable.
- No settings, environment variables or developer workflow are affected, and no feature gates are involved.
