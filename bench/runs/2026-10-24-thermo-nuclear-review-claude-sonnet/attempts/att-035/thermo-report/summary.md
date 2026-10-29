# Review of django/django#16631

### 1. Medium — `get_user()` raises `AttributeError` for duck-typed users that define `get_session_auth_hash` but not `get_session_auth_fallback_hash`

File: `django/contrib/auth/__init__.py`, lines ~202-221 (the new fallback branch); the new method is defined only in `django/contrib/auth/base_user.py`, lines ~141-143.

Evidence: `get_user()` still gates session verification on `hasattr(user, "get_session_auth_hash")`. The user object comes from `backend.get_user()`, and nothing requires it to subclass `AbstractBaseUser`. The patch adds a call to `user.get_session_auth_fallback_hash()` on the mismatch path. That method exists only on `AbstractBaseUser`. I grepped the tree: nothing else defines it, and no `hasattr` guard protects the call. `session_hash and any(... user.get_session_auth_fallback_hash())` runs whenever a session hash is present and does not match the current hash.

Failure: a project whose custom auth backend returns a user-like object with its own `get_session_auth_hash` (a documented customization point) used to have mismatched sessions flushed and the request treated as anonymous. After this change, any hash mismatch raises `AttributeError: ... has no attribute 'get_session_auth_fallback_hash'`, so the request returns a 500. A mismatch happens after a password change, or after a `SECRET_KEY` change with no fallbacks configured. In those cases the session should be invalidated cleanly. This is a backwards-incompatible regression in a 4.1.x bugfix release. The fix is to guard the call with `hasattr`/`getattr`, or give the fallback path its own check.

### 2. Low — Fallback verification silently ignores subclass overrides of `get_session_auth_hash()`

File: `django/contrib/auth/base_user.py`, lines ~136-152; `django/contrib/auth/__init__.py`, lines ~212-217.

Evidence: `get_session_auth_hash()` is a public override point, and `docs/topics/auth/customizing.txt` describes it as the method used for session invalidation. The patch computes the current hash through the overridable method. It computes the fallback hashes through the new `get_session_auth_fallback_hash()`, which calls the private `_get_session_auth_hash(secret=...)` directly. A subclass that overrides `get_session_auth_hash()` (for example, to add fields to the hashed value) therefore produces a current hash in one format and fallback hashes in another.

Failure: for such models the fallback comparison can never match, because the fallback hashes use the base formula. Sessions are still invalidated on key rotation, which is the bug this PR was meant to fix. This is not a new break, but the new docs say fallback verification is supported for `get_user()` without mentioning that overriding `get_session_auth_hash()` also requires overriding `get_session_auth_fallback_hash()`. Adding that note would document the limitation.

No other issues found in the diff. The `salted_hmac(secret=...)` signature exists, the `cycle_key()` plus rehash flow is correct, and the test passes for the base-user case.
