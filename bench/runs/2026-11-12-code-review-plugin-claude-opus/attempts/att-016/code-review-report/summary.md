# Code review: django/django#16631, "Fixed #34384 -- Fixed session validation when rotation secret keys."

Range reviewed: `9b224579875e30203d079cc2fee83b116d98eb78..2396933ca99c6bfb53bda9e53968760316646e01` (`main...review-head`).

Found 1 issue (kept after validation in step 6):

## 1. `get_user()` calls `get_session_auth_fallback_hash()` without checking it exists, so user objects that are not `AbstractBaseUser` raise `AttributeError`

- **Reason flagged:** bug (regression in how Django treats `get_session_auth_hash` as an optional, duck-typed method)
- **File and lines:** `django/contrib/auth/__init__.py` lines 209-221 (the call is on line 215); the method is defined only in `django/contrib/auth/base_user.py` lines 141-143.
- **Description:** `get_user()` only verifies the session if `hasattr(user, "get_session_auth_hash")` is true (line 200). `login()` (line 103) and `update_session_auth_hash()` (line 243) use the same check. Inside that block, the new code calls `user.get_session_auth_fallback_hash()` on line 215 with no check at all. That method exists only on `AbstractBaseUser`. The docs (`docs/topics/auth/default.txt`) explicitly support user models that "implement [their] own `get_session_auth_hash()` method" without subclassing `AbstractBaseUser`.
- **Why it fails:** take a user object (a custom model or one returned by a custom auth backend) that defines `get_session_auth_hash()` but doesn't inherit from `AbstractBaseUser`. If its stored session hash is non-empty but no longer matches, for example after a password change on another device:
  - `session_hash` is truthy and verification fails.
  - Evaluating `any(...)` calls `user.get_session_auth_fallback_hash()`, which raises `AttributeError`.

  Before this change, the session was flushed and `AnonymousUser` was returned. Now every request on that session returns a 500. This happens even when `SECRET_KEY_FALLBACKS` is empty, and it lands in the 4.1.8 bugfix release. A fix is to guard the fallback branch, for example with `hasattr(user, "get_session_auth_fallback_hash")`, or to use a `getattr` default that yields no hashes.
