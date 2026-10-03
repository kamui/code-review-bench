# Session authentication fallback review

## Finding: the fallback branch assumes a method the existing capability check does not require

**Evidence.** In `django/contrib/auth/__init__.py:200`, `get_user()` enters session validation when the returned object has `get_session_auth_hash()`. The new branch at `django/contrib/auth/__init__.py:213-216` then calls `user.get_session_auth_fallback_hash()` without checking that capability. The method is introduced only on `AbstractBaseUser` in `django/contrib/auth/base_user.py:141-143`.

Before this change, a user object with `get_session_auth_hash()` could participate in session validation without inheriting `AbstractBaseUser`; this path explicitly uses `hasattr()` rather than requiring a concrete model base. If its hash does not match the current key, `get_user()` used to flush the session and return `AnonymousUser`. With this change, that same object raises `AttributeError` while middleware resolves `request.user`. This is a compatibility regression in a shared authentication path, and the session mismatch has become an application error instead of a normal logout.

**Code-judo proposal.** Model fallback hashing as an optional capability adjacent to the existing hash capability, and keep the mismatch decision linear: current hash matches => keep the session; fallback capability exists and one hash matches => rotate the session key and replace the stored hash; otherwise => flush. In implementation terms, guard the fallback generator (or use a `getattr` default representing no fallbacks) before iterating. That retains the simple existing control flow and avoids forcing all legacy hash providers to implement a new method merely to preserve their previous mismatch behavior. Document the fallback hook as optional for custom user implementations.

**Action.** Add coverage using a backend user object with `get_session_auth_hash()` but no `get_session_auth_fallback_hash()`. Assert a stale hash follows the flush-and-anonymous path without raising. Keep the existing fallback-rotation coverage for `AbstractBaseUser` users.

**Verification status.** Static inspection confirms the unguarded call and the base-class-only definition. `git diff --check main...review-head` passed. Tests were not run.

**Measurements.** The changed files are 244 lines (`django/contrib/auth/__init__.py`), 167 lines (`django/contrib/auth/base_user.py`), and 164 lines (`tests/auth_tests/test_basic.py`) at the review head. No changed file approaches the skill's 1,000-line threshold. The implementation adds a small branch inside the existing auth-session validation path; the maintainability concern is the mismatched capability boundary, not branch volume.
