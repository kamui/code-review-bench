# Thermo Nuclear Review — django/django#16631

**Change:** "Fixed #34384 -- Fixed session validation when rotation secret keys."
**Range:** `9b224579875e30203d079cc2fee83b116d98eb78..2396933ca99c6bfb53bda9e53968760316646e01` (`main...review-head`)

## What the change does

`django.contrib.auth.get_user()` previously compared the session's stored auth hash only against `user.get_session_auth_hash()` computed with the current `SECRET_KEY`, so rotating the key (even with `SECRET_KEY_FALLBACKS` configured) logged every user out. The PR adds `AbstractBaseUser.get_session_auth_fallback_hash()` (a generator yielding the HMAC under each fallback secret, via a new private `_get_session_auth_hash(secret=None)` helper that passes `secret` to `salted_hmac`) and, when the primary comparison fails, has `get_user()` compare the stored hash against each fallback hash. On a fallback match it calls `request.session.cycle_key()` and rewrites `HASH_SESSION_KEY` with the current-secret hash; otherwise it flushes the session as before. Docs and a 4.1.8 release note are added, plus one test.

## Verification performed

* Read the full diff and traced `get_user()`, `login()`, `update_session_auth_hash()`, `AuthenticationMiddleware` (`request.user` lazy object and `request.auser()`), `SessionMiddleware.process_response()`, `SessionBase.cycle_key()/flush()`, and the `db` and `signed_cookies` session backends.
* Ran `auth_tests.test_basic auth_tests.test_middleware auth_tests.test_views auth_tests.test_remote_user sessions_tests` with `--settings=test_sqlite`: 555 tests, OK (2 skipped, 1 expected failure).
* Ran a scratch script (kept outside the clone) against the review head that reproduces findings 1 and 2 below.
* Security: accepting hashes computed with `SECRET_KEY_FALLBACKS` is consistent with how `django.core.signing` and `PasswordResetTokenGenerator` already trust those keys, so no new trust is introduced. `constant_time_compare` is used for every comparison. The `session_auth_hash` name is only referenced on the fallback branch when `session_hash` is truthy, so the apparent "possibly unbound" variable is not reachable. No feature-gate or devex concerns.

## Findings

### Finding 1 — High — `get_user()` now raises `AttributeError` for custom user models that implement `get_session_auth_hash()` without inheriting `AbstractBaseUser`

**File/lines:** `django/contrib/auth/__init__.py:200-216` (the new fallback branch, specifically the unconditional call on lines 213-216).

**Evidence:** The guard on line 200 is `hasattr(user, "get_session_auth_hash")`, but the new code on lines 213-216 unconditionally calls `user.get_session_auth_fallback_hash()`, a method that only exists on `AbstractBaseUser` (`django/contrib/auth/base_user.py:141`). Django explicitly documents the configuration where a custom `AUTH_USER_MODEL` does *not* inherit `AbstractBaseUser` but "implements its own `get_session_auth_hash()` method" (`docs/topics/auth/default.txt:920-923`). For such a model, any session whose stored hash does not match the current hash — the normal state after a password change in another session, after `SECRET_KEY` is changed, or when a stale cookie is presented — reaches the fallback branch and raises `AttributeError: '...' object has no attribute 'get_session_auth_fallback_hash'`. Before this PR that path flushed the session and returned `AnonymousUser`. Reproduced with a scratch script: a non-`AbstractBaseUser` user class defining only `get_session_auth_hash()` returned the user when the hash matched and raised `AttributeError` when it did not.

**Failure:** Because `AuthenticationMiddleware` wraps `get_user()` in `SimpleLazyObject` for `request.user` (and `sync_to_async(auth.get_user)` for `request.auser()`), the exception surfaces on the first `request.user` access in any view, template, or middleware — i.e. a 500 on every request from such a session until the user clears their cookie, instead of a silent logout. The change is being backported to the 4.1.8 patch release, so this regression lands on stable-release users of a documented customization. The fix is to guard the fallback lookup (e.g. `hasattr(user, "get_session_auth_fallback_hash")`, or `getattr(user, "get_session_auth_fallback_hash", lambda: ())()`), and to add a test covering a user model that defines only `get_session_auth_hash()`.

### Finding 2 — Medium — `cycle_key()` in `get_user()` deletes the old server-side session record, so concurrent requests from the same browser right after rotation are logged out or have their cookie deleted

**File/lines:** `django/contrib/auth/__init__.py:217` (`request.session.cycle_key()`).

**Evidence:** `SessionBase.cycle_key()` (`django/contrib/sessions/backends/base.py:298-307`) creates a new session record and then `delete(key)`s the old one. `get_user()` runs lazily inside each request that touches `request.user`, so after a key rotation *every* request still carrying the old-hash session executes this branch. With server-side backends (`db`, `cached_db`, `cache`, `file`), request A deletes the old session row while request B, carrying the same cookie, is still in flight. If B loads its session after A's delete, `SessionStore.load()` finds no row, sets `_session_key = None` and returns `{}` (`django/contrib/sessions/backends/db.py:30-43`), so B's `get_user()` sees no `SESSION_KEY` and returns `AnonymousUser`. Then `SessionMiddleware.process_response()` observes `is_empty()` true with the cookie present and emits `delete_cookie(SESSION_COOKIE_NAME)` (`django/contrib/sessions/middleware.py:36-43`). Reproduced in the scratch script with the `db` backend: request A → authenticated with a new key, old row gone; request C loading afterwards → `AnonymousUser`, `is_empty()` True. Requests that loaded before the delete each create their own new row, orphaning the others.

**Failure:** The first burst of parallel requests after a rotation (typical for SPAs, dashboards, or pages issuing several XHR/fetch calls at once) will intermittently return unauthenticated responses (401/403/redirect-to-login) for some of them, and if the `AnonymousUser` response arrives last its `Set-Cookie` deletion wins and the user is fully logged out — exactly the outcome the ticket is trying to prevent, now occurring nondeterministically. The session key for server-side backends is a random string not derived from `SECRET_KEY`, so there is no security reason to cycle it here; `update_session_auth_hash()` (same file, lines 233-244) handles the analogous "hash changed, keep session" case by simply assigning `request.session[HASH_SESSION_KEY]` without cycling. Dropping the `cycle_key()` call (keeping the `HASH_SESSION_KEY` rewrite, which already marks the session modified) removes the race; for `signed_cookies` the save re-signs the cookie with the new key regardless. If the author does want key cycling for some reason, it should be stated and the race acknowledged in the docs.

### Finding 3 — Low — Fallback verification bypasses an overridden `get_session_auth_hash()`, so the fix does not work for models that customise the hash

**File/lines:** `django/contrib/auth/base_user.py:141-143` (`get_session_auth_fallback_hash`) and `docs/topics/auth/customizing.txt:725-730`.

**Evidence:** `get_session_auth_fallback_hash()` always calls the private `_get_session_auth_hash(secret=...)`, which hashes only `self.password` with the fixed `AbstractBaseUser` salt. An `AbstractBaseUser` subclass that overrides `get_session_auth_hash()` (e.g. to also bind the session to the email or a "sessions_valid_from" field — a pattern the docs invite in `docs/topics/auth/default.txt:920-923`) stores a hash produced by the override, so none of the fallback hashes can ever match it. Such users are still logged out on rotation, and nothing in the new docs tells them they must also override `get_session_auth_fallback_hash()` to keep the two in sync.

**Failure:** Not a regression (behaviour equals pre-PR), but the advertised bugfix is silently ineffective for a documented customization, and the new public method's docstring/docs do not mention the coupling. Either compute the fallbacks through the public hook (e.g. have `get_session_auth_hash()` accept an optional secret, or document that overriders must override both), or add a note in `customizing.txt`.

## Non-findings considered

* `login()` still compares the stored hash only against the current-secret hash and flushes on mismatch; after rotation this only matters if `login()` runs before anything touches `request.user`, and it results in a fresh session rather than an error. Unchanged code; not reported.
* Timing: `any(...)` short-circuits over fallbacks, which could reveal which fallback matched, but every individual comparison is constant-time and the fallback list is public configuration, so there is no practical leak.
* Test `test_get_user_fallback_secret` evaluates `settings.SECRET_KEY` before `override_settings` is entered, so the fallback list correctly contains the original key; the second block restores `SECRET_KEY_FALLBACKS` to `[]` and verifies the rewritten hash. The test passes and exercises the intended path, though it does not cover Finding 1 or 2.
