# Code review — django/django#16631

"Fixed #34384 -- Fixed session validation when rotation secret keys." (range `main...review-head`, 9b22457987..2396933ca9)

Step 1 decided the PR needed review. Step 2 found no CLAUDE.md files that apply. Step 6 kept 1 issue.

## Issues

### 1. `get_user()` raises `AttributeError` for user models that implement only `get_session_auth_hash()`

- **Reason flagged:** bug (backward-compatibility regression)
- **File / lines:** `django/contrib/auth/__init__.py` lines 200-221 (guard at line 200, unguarded call at line 215); the method is defined only on `AbstractBaseUser` in `django/contrib/auth/base_user.py` lines 141-143.
- **Description:** `get_user()` decides whether to verify the session with `hasattr(user, "get_session_auth_hash")`. When the stored hash is non-empty and doesn't match, the new fallback branch then calls `user.get_session_auth_fallback_hash()` without checking that it exists. Only `AbstractBaseUser` defines that method.
- **Why it fails:** Django supports user models that implement `get_session_auth_hash()` without inheriting from `AbstractBaseUser`. `docs/topics/auth/default.txt` says "If your AUTH_USER_MODEL inherits from AbstractBaseUser or implements its own get_session_auth_hash() method…", and the code checks with `hasattr` at lines 103, 200 and 243. For such a user, a stale session hash (for example after a password change) used to flush the session and return `AnonymousUser`. It now raises `AttributeError: '<User>' object has no attribute 'get_session_auth_fallback_hash'`, which turns every request with that session into a 500. The validation step reproduced this: review-head raises `AttributeError`, while main flushes the session and returns `AnonymousUser`. This lands in a 4.1.8 patch release. A fix is to guard the fallback with `hasattr`/`getattr` and treat a missing method as having no fallback hashes.
