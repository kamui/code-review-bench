# Thermo-nuclear review: django/django#16631

Range: `9b224579875e30203d079cc2fee83b116d98eb78..2396933ca99c6bfb53bda9e53968760316646e01`. I ran `auth_tests.test_basic`, `auth_tests.test_views` and `auth_tests.test_middleware` against the head, and they pass. The findings below are not covered by those tests.

## Finding 1 (Medium): `get_user()` raises `AttributeError` for user objects that define `get_session_auth_hash` but not `get_session_auth_fallback_hash`

File: `django/contrib/auth/__init__.py`, lines 199-221. The related definition is in `django/contrib/auth/base_user.py`, lines 135-145.

Evidence: `get_user()` still gates session verification on `hasattr(user, "get_session_auth_hash")`. Inside that gate, when the primary hash does not match, the new code calls `user.get_session_auth_fallback_hash()` unconditionally. It does this whenever `session_hash` is truthy. The new method exists only on `AbstractBaseUser`. Any user object returned by an auth backend that implements `get_session_auth_hash` without subclassing `AbstractBaseUser` has no such method. Examples are duck-typed or proxy users, and test doubles. For those objects the call raises `AttributeError`.

Failure: Before this PR, a stale or mismatched session hash led to `session.flush()` and an anonymous user. After it, the same condition (for example a password change on another device) crashes the request with a 500 from `AuthenticationMiddleware`/`request.user`. This is a regression for a configuration the `hasattr` check was written to support. It also happens even when `SECRET_KEY_FALLBACKS` is empty, because the method is looked up before the generator runs. The fix is to guard with `hasattr`/`getattr`, or to compute the fallbacks in a way that tolerates a missing method.

## Finding 2 (Low/Medium): Fallback verification ignores user-model overrides of `get_session_auth_hash`

File: `django/contrib/auth/base_user.py`, lines 135-153. It is used from `django/contrib/auth/__init__.py`, lines 211-219.

Evidence: `get_session_auth_hash()` is a documented customization point, because subclasses override it to change what goes into the session hash. The new `get_session_auth_fallback_hash()` calls the private `_get_session_auth_hash(secret=...)`, which always HMACs `self.password` with the base salt. It never routes through the subclass override. A custom user model that overrides `get_session_auth_hash` (for example to include other fields) will produce session hashes that the fallback generator can never reproduce.

Failure: The bug this PR sets out to fix, sessions invalidated on `SECRET_KEY` rotation, remains for such projects. Rotation still logs everyone out, silently, because `get_user()` falls through to `flush()`. Fixing it requires overriding a second, newly introduced method, and the docs do not tell users to do that. The reverse case is also possible. If an override keeps the same input data as the base, the generator computes hashes from a different algorithm than the override, so the two can disagree.

## Notes (not separately actionable)

- `docs/topics/auth/customizing.txt` marks `get_session_auth_fallback_hash` as `versionadded:: 4.1.8`. This adds a new public API in a patch release, which is questionable for Django's backport policy. Because of Finding 2 it also becomes an extra method that custom user models must now override.
