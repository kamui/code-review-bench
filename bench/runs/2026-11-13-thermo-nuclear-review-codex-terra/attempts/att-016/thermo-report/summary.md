# Review findings

**[P1] Preserve invalidation for user models without the new fallback method** — `django/contrib/auth/__init__.py:213-215`

`get_user()` supports user objects that implement `get_session_auth_hash()` without inheriting `AbstractBaseUser` (the session invalidation documentation explicitly describes this option). Those existing models do not have the newly introduced `get_session_auth_fallback_hash()` method. Whenever a nonempty session hash no longer matches, such as after a password change, this call raises `AttributeError` instead of flushing the session and returning an anonymous user. This occurs even with the default empty `SECRET_KEY_FALLBACKS`, so affected users receive server errors on requests using their old sessions after upgrading.

**[P2] Compute fallback hashes using the custom session hash scheme** — `django/contrib/auth/base_user.py:141-143`

An `AbstractBaseUser` subclass can override `get_session_auth_hash()` to incorporate additional user state or use a different hash scheme. The inherited fallback method bypasses that override and always computes the base password-only HMAC. Consequently, a session created using the custom method under the old secret cannot match any fallback after secret rotation and is flushed, defeating the rotation support for those user models. In addition, accepting the base hash can bypass custom invalidation state if a previously issued session used that base hash and the custom method later changes its result to invalidate it.

The focused `auth_tests.test_basic.TestGetUser` tests passed (3 tests); they exercise the built-in user model.
