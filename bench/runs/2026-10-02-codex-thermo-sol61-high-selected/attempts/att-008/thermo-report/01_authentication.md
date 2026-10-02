# Authentication: fallback verification and the custom-user boundary

## Judgment

One actionable compatibility defect is introduced at the user-method boundary. The fallback feature otherwise belongs in this authentication flow. The user model remains responsible for deriving an HMAC from its password, and authentication remains responsible for deciding whether to preserve, migrate, or reject a session.

## Finding: preserve the optional fallback-hash boundary

The changed invocation at `django/contrib/auth/__init__.py:213–215` assumes a stronger protocol than the surrounding code establishes. At line 200, the only required capability is `get_session_auth_hash()`. Once a nonempty stored hash fails comparison with the current hash, line 215 invokes `get_session_auth_fallback_hash()` regardless of whether that method exists or whether any fallback secrets are configured.

This is an existing supported extension point, not a hypothetical misuse. The “Session invalidation on password change” section of `docs/topics/auth/default.txt:920` explicitly covers a user model that inherits `AbstractBaseUser` **or implements its own** `get_session_auth_hash()`. The `login()` and `update_session_auth_hash()` functions in the same authentication module also continue to discover the older method with `hasattr()`. No existing caller contract requires every such user object to inherit the base class.

The base revision rejected a stale hash by flushing the session and setting `user = None`. The head instead tries a method that those custom implementations need not have. A custom backend returning such a user will raise `AttributeError` when a password change makes the stored hash stale. An application accessing the lazily resolved `request.user` can consequently fail its request instead of treating the stale session as anonymous. The failure also occurs with `SECRET_KEY_FALLBACKS=[]`: Python must call the missing method to obtain the iterable before `any()` can decide that there are no candidates.

The precise affected source anchor is `django/contrib/auth/__init__.py:213–215`. Severity is P2: the failure is conditional on a supported custom-user implementation and a nonempty invalid hash, but it changes ordinary rejection into an exception and should be corrected before merging a patch release.

## Source-level verification

The trace requires no database-specific assumptions. Given a configured backend and a user object with only the existing hash capability:

```python
class ExistingCustomUser:
    def get_session_auth_hash(self):
        return "current-hash"
```

Set the request session's authentication hash to `"stale-hash"` and let the configured backend return that user. The capability check at line 200 succeeds. The stored hash is truthy, so `session_auth_hash` is assigned. The constant-time comparison is false. The left operand at line 213 is truthy, so evaluating the generator expression must resolve `user.get_session_auth_fallback_hash()`. That attribute is absent, and execution never reaches `flush()` at line 220.

A matching current hash avoids the new invocation and remains accepted. A missing or empty stored hash short-circuits the fallback expression and remains rejected. The defect therefore concerns the nonempty mismatch case, rather than every session for a custom user.

This trace was verified against the committed source and the base implementation, not executed as an added test. No scratch reproducer was injected into the checkout, and no remedy was applied. The passing repository tests use users that inherit the new method and therefore do not refute this finding.

Evidence was read with `git diff main...review-head`, `git show main:django/contrib/auth/__init__.py`, `nl -ba django/contrib/auth/__init__.py`, and focused `sed`/`rg` reads of the authentication implementation and the documented custom-user contract.

## Worked code-judo proposal

Make the fallback capability explicitly optional where it crosses into authentication. Keep the existing success, migration, and rejection outcomes. The following replaces only the current fallback condition inside `if not session_hash_verified:`:

```python
fallback_hash_getter = getattr(user, "get_session_auth_fallback_hash", None)
if (
    session_hash
    and fallback_hash_getter is not None
    and any(
        constant_time_compare(session_hash, fallback_auth_hash)
        for fallback_auth_hash in fallback_hash_getter()
    )
):
    request.session.cycle_key()
    request.session[HASH_SESSION_KEY] = session_auth_hash
else:
    request.session.flush()
    user = None
```

This is a proposal, not an applied or tested patch. Its `None` denotes absence of an optional method; it does not add a mode to the hashing algorithm. A direct `hasattr(user, "get_session_auth_fallback_hash")` guard in the existing condition is another equally small implementation. Do not catch `AttributeError` around the whole fallback computation: that would also hide defects raised inside a custom implementation.

The structural move is to restore the old protocol's extension boundary, rather than force inheritance or scatter settings checks across `login()`, `get_user()`, and session stores. Checking only whether fallback settings are nonempty would leave the same exception when fallback keys are configured. Calling the base class's HMAC implementation on arbitrary custom users would violate their chosen hashing contract.

The remedy preserves lazy fallback computation, short-circuiting after the first matching hash, constant-time comparisons, current-key fast-path behavior, and the existing session rotation plus current-hash migration. For an absent fallback method, it restores the base revision's rejection outcome.

## Why broader simplification is not a blocker

The extraction in `django/contrib/auth/base_user.py:135–152` is useful: both primary and fallback methods now use one implementation of the salt, password input, algorithm, and digest formatting. The primary wrapper preserves an established no-argument public API, while the private helper adds an explicit secret parameter. These wrappers earn their place through API compatibility and shared cryptographic behavior.

Replacing the primary comparison and fallback comparison with one `any()` over all hashes would hide which key matched. That distinction matters: a current-key match should preserve the session key, whereas a fallback match must migrate the session. Recovering the distinction would require another flag or an indexed result, so the apparent simplification would not delete the underlying concepts. Reusing `Signer` directly would likewise change the data format and mix a signed-value protocol with this password-HMAC protocol.

The added branches reflect three real outcomes: absent/invalid authentication evidence, a primary-key match, and a fallback-key match. The fact that `session_auth_hash` is only assigned for a truthy stored hash is currently safe because the later assignment is guarded by that same truthiness. This could be made easier to scan in a future local refactor, but it is not an unbound-variable bug and does not warrant a separate actionable finding.

The design should retain customized HMAC ownership on the model. Models overriding the primary method with different inputs must provide compatible fallback generation to support rotation under that customization. The inherited method is documented specifically as a password HMAC. The review does not treat support for every custom hashing algorithm as an additional defect or propose global settings mutation to emulate an old secret.

## Remediation verification

Add a regression test whose backend returns a user with `get_session_auth_hash()` and no fallback method. Use a nonempty stale authentication hash and the default empty fallback setting; assert that `get_user()` returns `AnonymousUser` and clears the session. Include a matching-hash case to retain existing success behavior. A backend stub can be scoped to the test rather than adding a production user hierarchy.

After the guard is implemented, retain the PR's `TestGetUser.test_get_user_fallback_secret` check so fallback matches still rotate the key and update the authentication hash. The already executed authentication selection and its status are recorded in the verification detail. No new test or production code was written into the read-only checkout during this review.
