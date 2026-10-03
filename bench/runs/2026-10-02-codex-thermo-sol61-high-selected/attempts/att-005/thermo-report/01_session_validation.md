# Session validation and migration

## Scope and judgment

Reviewed `django/contrib/auth/__init__.py` against the pinned base, plus
login, logout, session-hash updates, backend lookup, session stores, and
session middleware. The main concern is an implicit widening of the
backend user's protocol in a failure path. Backend lookup and session
migration still belong in auth; moving password-HMAC knowledge into
signing or session storage would weaken the boundary.

## Finding: [P2] Preserve the optional fallback-hash contract for custom users

In `django/contrib/auth/__init__.py:213–215`, the fallback branch calls `user.get_session_auth_fallback_hash()` after checking only for `get_session_auth_hash()`. Django explicitly supports user models that implement their own existing hash method without inheriting `AbstractBaseUser` (`docs/topics/auth/default.txt:920–927`). For such a user, any nonempty stale session hash now raises `AttributeError` instead of flushing the session and returning `AnonymousUser`; this happens even with the default empty `SECRET_KEY_FALLBACKS`, because Python evaluates the generator's iterable before `any()` can see an empty sequence. A password change is enough to trigger this regression. Treat fallback hashing as a separately optional capability: when the method is absent, take the existing invalid-session path, and add a regression test using a backend user with only the old method. Keep that capability check inside the authentication validation boundary rather than requiring every existing custom user to acquire a new method.

## Evidence and verification status

The old implementation, visible in `git diff main...review-head`, checked
only `hasattr(user, "get_session_auth_hash")`. A nonempty hash mismatch
immediately flushed the session and set `user = None`. The return at the
end supplied `AnonymousUser()`.

The head keeps that same capability check at line 200 but calls the new
method without a corresponding check at line 215. This is not a
requirement that can be inferred from the existing method: the new
fallback method is added only to `AbstractBaseUser`, while the documented
old contract permits independently implemented user models.

The decisive trace is:

1. A configured backend returns a user with `get_session_auth_hash()`
   and no `get_session_auth_fallback_hash()`.
2. The request has a nonempty stored authentication hash from before a
   password change.
3. Lines 205–208 compute the new hash and reject the old one.
4. Line 213's `session_hash` condition is true, so evaluating the
   generator requires calling the method at line 215.
5. Attribute lookup raises before the flush at line 220. The configured
   fallback list is never consulted and can be empty.

The same issue occurs on other nonempty mismatches, including key
rotation and corrupted auth hashes. Missing or empty hashes do not reach
the new method, and current-key matches do not reach it either. These
bounds are part of the finding; there is no claim that every custom-user
request fails.

Verification is static. The 87-test run passed, but its backend tests do
not establish this old-method-only mismatch case. No custom test was
injected into the immutable checkout.

## Measurements and commands

`git diff main...review-head --numstat` reports 19 additions and five
deletions in this file. `git show main:django/contrib/auth/__init__.py |
wc -l` reports 230 lines; `wc -l django/contrib/auth/__init__.py` reports
244 at head. There is no size threshold crossing.

Numbered source was inspected with `nl -ba`. Searches for
`get_session_auth_hash`, `get_session_auth_fallback_hash`,
`HASH_SESSION_KEY`, and `SECRET_KEY_FALLBACKS` traced the relevant
callers and tests. The documentation evidence is
`docs/topics/auth/default.txt:920–927`; the capability check and new
call are `django/contrib/auth/__init__.py:200–215`.

The change adds missing-hash branching, a verification flag, and nested
fallback migration to a function already nested inside backend lookup.
The complexity is local and bounded, but storing the flag and assigning
the current hash in a separate branch makes the reader prove an
incidental invariant: fallback acceptance implies that the current hash
was assigned. It is currently true because both depend on a truthy
stored hash; there is no unbound-variable defect in the submitted code.

## Worked code-judo proposal

A compatible minimal fix checks the new capability before calling it
and treats its absence as invalidation. If restructuring this block,
keep it as a private auth helper with a narrow documented result:
true means the session hash is accepted and any migration is complete;
false means the caller must invalidate it. The helper is not pure, so
its name and docstring should make that mutation explicit.

For example, this proposal preserves current-key priority, laziness,
the missing-hash behavior, and the submitted mutation order:

```python
def _verify_and_refresh_session_auth_hash(request, user):
    """Accept the session hash, refreshing it after a fallback match."""
    session_hash = request.session.get(HASH_SESSION_KEY)
    if not session_hash:
        return False

    current_hash = user.get_session_auth_hash()
    if constant_time_compare(session_hash, current_hash):
        return True

    get_fallback_hashes = getattr(user, "get_session_auth_fallback_hash", None)
    if get_fallback_hashes is None:
        return False

    if not any(
        constant_time_compare(session_hash, fallback_hash)
        for fallback_hash in get_fallback_hashes()
    ):
        return False

    request.session.cycle_key()
    request.session[HASH_SESSION_KEY] = current_hash
    return True
```

The existing backend-resolution block then ends with:

```python
if hasattr(user, "get_session_auth_hash"):
    if not _verify_and_refresh_session_auth_hash(request, user):
        request.session.flush()
        user = None
```

This removes the mutable verification flag and conditional lifetime of
the current hash from backend orchestration. It centralizes the optional
fallback boundary without introducing a policy object, mode enum, or
cross-package protocol. It costs a helper, so the smaller guarded inline
fix is also acceptable if the maintainers prefer the existing locality.
No helper extraction is necessary to establish finding one's behavior.

Do not catch `AttributeError` around the whole fallback calculation:
that could hide an exception raised inside an actual implementation.
Do not use a missing fallback implementation to accept an unverified
session. The compatible default is the previous flush-and-anonymous
behavior.

This proposal is review text only; it was not applied or executed.

## Session lifecycle assessment

`SessionBase.cycle_key()` retains session data while creating a new key
and deleting the old key. The auth code follows that with assignment of
the current authentication hash. `SessionMiddleware.process_response()`
normally saves modified sessions and sends the updated cookie.
The ordering matches the existing `update_session_auth_hash()` pattern;
there is no new atomicity finding just because those two operations are
sequential.

Database creation can precede the final updated-hash save, and cookie
storage implements key cycling differently. These are existing session
abstractions, not evidence that this patch must introduce a transaction
or backend-specific migration code. Their behavior was inspected, not
runtime-tested in a backend matrix.

## Remediation checks

A focused regression should use a backend returning a user with the old
hash capability alone, arrange a nonempty mismatching session hash, and
assert both anonymous return and flushing. Run it with empty fallbacks
as well as configured fallbacks. Also assert that a matching current hash
continues to authenticate this user without demanding the new method.

For default users, preserve current-key acceptance without cycling,
fallback acceptance with cycling and rehashing, no-match invalidation,
and missing-hash invalidation. A later fallback match and password change
under rotation are useful rejection/iteration checks. These are proposed
checks, not executed verification.

