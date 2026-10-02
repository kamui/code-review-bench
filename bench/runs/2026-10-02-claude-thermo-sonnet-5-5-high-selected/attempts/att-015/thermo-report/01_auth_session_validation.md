# Detail: auth session validation with SECRET_KEY_FALLBACKS

Scope: `django/contrib/auth/__init__.py`, `django/contrib/auth/base_user.py`, `tests/auth_tests/test_basic.py`, and the three docs files. Reference code: `django/contrib/auth/tokens.py`, `django/utils/crypto.py`.

## Commands and measurements

- `git diff main...review-head` showed 6 files, +69/-6 lines.
- `wc -l django/contrib/auth/__init__.py` gives 244 lines, so there is no 1k-line crossing.
- `PYTHONPATH=<clone> venv/bin/python tests/runtests.py auth_tests.test_basic auth_tests.test_middleware --settings=test_sqlite` ran 17 tests, OK.
- `grep -n SECRET_KEY_FALLBACKS -r django` shows consumers in `tokens.py`, `signing.py`, security checks and now `base_user.py`.

Verification status for every finding: confirmed by reading the code at the pinned head. The F2 `AttributeError` is derived from reading the control flow and was not executed.

## 1. get_user() control flow (F1)

Control flow at `django/contrib/auth/__init__.py:199-222`: `session_hash` is read at line 201. If it is falsy, `session_hash_verified = False`. Otherwise `session_auth_hash` is computed at line 205 and compared. When unverified, line 213 requires `session_hash` again before calling any fallback. The inner branch uses `session_auth_hash` at line 217, which is bound only on the path where `session_hash` was truthy. The two `session_hash` truthiness checks must stay in sync or `UnboundLocalError` follows.

Worked code-judo proposal. Keep `get_user()` flat:

```python
if hasattr(user, "get_session_auth_hash"):
    if not _session_hash_is_valid(request, user):
        request.session.flush()
        user = None
```

with a helper:

```python
def _session_hash_is_valid(request, user):
    session_hash = request.session.get(HASH_SESSION_KEY)
    if not session_hash:
        return False
    current = user.get_session_auth_hash()
    if constant_time_compare(session_hash, current):
        return True
    if any(constant_time_compare(session_hash, h) for h in _fallback_hashes(user)):
        request.session.cycle_key()
        request.session[HASH_SESSION_KEY] = current
        return True
    return False
```

This removes `session_hash_verified`, the duplicated truthiness guard and two nesting levels, and `current` is always bound before use. The side effect (cycling) is visible in one named place.

## 2. Contract of the fallback call (F2)

The `hasattr(user, "get_session_auth_hash")` guard exists because `get_user()` and `login()` accept user objects that do not subclass `AbstractBaseUser`. The new call to `user.get_session_auth_fallback_hash()` at line 215 is unguarded. Scenario: such a user object, session hash that no longer matches (password change), so `session_hash` is truthy and unverified, so `any(...)` evaluates the generator expression, whose iterable is `user.get_session_auth_fallback_hash()`, which raises `AttributeError`. Before the patch the request ended with a flush and an anonymous user. It now errors.

Second scenario: a custom `AbstractBaseUser` subclass overrides `get_session_auth_hash` to use a different salt, algorithm or extra fields. The inherited `get_session_auth_fallback_hash` calls the private `_get_session_auth_hash(secret=...)`, which bypasses the override, so no fallback ever matches and rotation logs these users out silently. Remedy options: guard with `getattr`/`hasattr` and document that overriders must override both methods, or define the fallback in terms of the public method (for example by passing the secret into an overridable hook).

## 3. Wrapper and public API growth (F3)

`base_user.py:135-150` now has `get_session_auth_hash()` calling `_get_session_auth_hash()`, plus `get_session_auth_fallback_hash()` looping over `settings.SECRET_KEY_FALLBACKS`. `base_user.py` also gains a `django.conf.settings` import for this. The documentation adds `get_session_auth_fallback_hash` as a public, versionadded API in `docs/topics/auth/customizing.txt`. Compare `tokens.py`, where `secret` and `secret_fallbacks` are properties on the generator object, and `check_token` iterates them. The model never learns about fallbacks. Applying that shape here means the auth module owns the secret iteration, and the model offers a single primitive (`get_session_auth_hash`, optionally accepting a secret).

## 4. Short-circuit generator (F4)

`any(constant_time_compare(...) for ... in user.get_session_auth_fallback_hash())` is lazy at both levels, so work stops at the first match. This is fine functionally. The point is only that this logic plus its explanatory comment is better as a named helper (see section 1).

## 5. Test gaps (F5)

`test_get_user_fallback_secret` (`tests/auth_tests/test_basic.py:143`) asserts the user resolves and the session key changes. It does not assert `request.session[HASH_SESSION_KEY]` equals the new-secret hash. The second `override_settings(SECRET_KEY="newsecret")` block does catch a missing rewrite, because with no fallback an un-rewritten hash would flush and return `AnonymousUser`. It checks the outcome indirectly, though, and says nothing about what was stored. Missing cases: tampered hash with fallbacks configured must flush and return `AnonymousUser`; missing hash with fallbacks configured; non-`AbstractBaseUser` user object (F2); custom `get_session_auth_hash` override.
