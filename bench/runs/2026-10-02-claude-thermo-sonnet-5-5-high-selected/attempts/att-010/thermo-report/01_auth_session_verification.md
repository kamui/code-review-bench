# Detail report 01: django.contrib.auth session verification and fallback hashes

Scope: `django/contrib/auth/__init__.py` (`get_user`), `django/contrib/auth/base_user.py` (`AbstractBaseUser`), the docs, and `tests/auth_tests/test_basic.py`. Range: `9b224579875e30203d079cc2fee83b116d98eb78..2396933ca99c6bfb53bda9e53968760316646e01`.

## Measurements and commands

- `git diff main...review-head` shows the six changed files. The code changes are +19/-5 in `__init__.py` and +9/-0 in `base_user.py`. Neither file is near 1000 lines (244 and 167 lines), so the file-size rule is not triggered.
- `PYTHONPATH=. ../clone-cache/venv/bin/python tests/runtests.py auth_tests.test_basic --settings=test_sqlite` passes (OK). The suite does not cover the findings below.
- `grep -rn "get_session_auth_hash" django` shows the hook is called at three sites in `django/contrib/auth/__init__.py` (lines 103, 205, 244). All three are guarded with `hasattr(user, "get_session_auth_hash")`. This is the documented duck-typing contract: a user object only needs that one method. The new fallback call is the only unguarded one.

## Finding 1: unguarded call to a new method breaks the duck-typed user contract (verified by reading, not executed)

Evidence: `__init__.py:200` guards on `hasattr(user, "get_session_auth_hash")`. Lines 213-216 then call `user.get_session_auth_fallback_hash()` without any guard. The method exists only on `AbstractBaseUser` (`base_user.py:141`).

Problem: the guard exists because `get_user` supports user objects that define `get_session_auth_hash` but do not subclass `AbstractBaseUser`. Before this change, a session hash mismatch for such an object ended in `session.flush()` and an anonymous user. After this change, a mismatch reaches `user.get_session_auth_fallback_hash()` and raises `AttributeError` out of `get_user`, which is called from `AuthenticationMiddleware` on request handling. That is a 500 where there used to be a clean logout. The same guard-versus-call mismatch applies to a model that subclasses `AbstractBaseUser` but overrides `get_session_auth_hash`. This pattern is common, for example to mix in a per-user token. Its override is never consulted for fallback hashes, because `get_session_auth_fallback_hash` goes straight to the private `_get_session_auth_hash`. Rotation then silently does not work for those models, and sessions are invalidated again, which is the bug the PR sets out to fix.

Code-judo proposal: stop growing the user-model API. Keep `get_session_auth_hash()` as the single extension point, and let `get_user` ask the hook for a fallback-secret variant only through that hook. For example, give the existing method an optional `secret=None` argument that is forwarded to `salted_hmac`. `get_user` can then check candidates with `user.get_session_auth_hash(secret=s)` for `s` in `settings.SECRET_KEY_FALLBACKS`. A subclass override that accepts `**kwargs` or `secret` keeps working. If that is considered too invasive for a backport, the minimum is to guard the call, for example `getattr(user, "get_session_auth_fallback_hash", None)`, and to document that overriders must override both methods.

## Finding 2: `get_user` verification block became a tangled, order-dependent branch

Evidence: `__init__.py:201-221`. The old three-line verify-or-flush block is now a nested if/else with a boolean flag (`session_hash_verified`) that is only meaningful inside the `else`. The variable `session_auth_hash` is assigned only in the `else` arm at line 205. It is read at line 220 inside a different arm, protected only by the `session_hash and any(...)` short-circuit. Correctness depends on the invariant "truthy `session_hash` implies the `else` arm ran". A reader has to prove that, and a later edit (for example, restructuring the first `if`) can turn it into an `UnboundLocalError`.

Problem: this is a new special-case ladder inserted into the middle of an already busy function. The function now does four jobs inline: backend lookup, current-secret verification, fallback verification, and session mutation (`cycle_key` or `flush`). `session_hash_verified` is computed and then immediately negated, so it earns nothing.

Code-judo proposal: pull the decision into one pure helper and leave `get_user` with a flat flow.

```python
def _session_hash_is_valid(user, session_hash):
    """Return (valid, needs_upgrade)."""
    if not session_hash:
        return False, False
    if constant_time_compare(session_hash, user.get_session_auth_hash()):
        return True, False
    return any(
        constant_time_compare(session_hash, h)
        for h in user.get_session_auth_fallback_hash()
    ), True
```

`get_user` then reads: if invalid, flush and set `user = None`. If it needs an upgrade, rewrite `HASH_SESSION_KEY`. The unbound-variable coupling and the flag disappear. The fallback iteration can also live here instead of on the model (see Finding 3).

## Finding 3: new public, documented model API plus a private pass-through for a cross-cutting concern

Evidence: `base_user.py:135-153`. `get_session_auth_hash` becomes a one-line pass-through to `_get_session_auth_hash()`. A new public generator `get_session_auth_fallback_hash` reads `settings.SECRET_KEY_FALLBACKS` inside the model. `base_user.py` gains a `django.conf.settings` import. `docs/topics/auth/customizing.txt` adds the generator to the permanent `AbstractBaseUser` reference with `versionadded:: 4.1.8`.

Problem: the model now knows about the key-rotation policy, and a bugfix release permanently grows the public user-model surface by a method that exists only to serve one caller in `get_user`. The private `_get_session_auth_hash(secret=None)` is an extra layer whose only purpose is to thread a parameter. The codebase already centralises "current key plus fallbacks" in `django/core/signing.py` and `django/contrib/auth/tokens.py` (the `secret_fallbacks` pattern). This diff adds a third, differently shaped mechanism: a generator on the model, with no `secret_fallbacks` parameter and no shared helper.

Code-judo proposal: add the optional `secret` argument directly on `get_session_auth_hash` (removing the private wrapper) and iterate the fallbacks in `get_user` or in the helper from Finding 2. That deletes one public method, one private method, one doc entry, and the settings import in `base_user.py`. If a model-level method is still wanted, it should be private (`_`-prefixed) and not documented as public API in a patch release.

## Finding 4: `cycle_key()` inside a read path is unnecessary and has side effects

Evidence: `__init__.py:219`. On a fallback match, `get_user` calls `request.session.cycle_key()` and then rewrites the hash. `get_user` is the lazy `request.user` accessor, used by `AuthenticationMiddleware` and by `auser()`.

Problem: the only thing that needs to change is the stored `HASH_SESSION_KEY`, so that it is signed with the current secret. A session-key rotation is a larger operation, because it creates a new session row and deletes the old one. Two parallel requests carrying the same old cookie (common for pages with several XHR or asset requests) race: the first cycles the key and deletes the old session, the second can then load an empty session and treat the user as anonymous. The result is a visible spurious logout during exactly the rotation window the feature is meant to make seamless. The new key also only reaches the client if the session is saved and the cookie rewritten later in the response cycle. No code in `get_user` guarantees that, and the test never checks it. Nothing in the changed code or docs explains why a key cycle is needed; `update_session_auth_hash` does it for the different reason of password change.

Code-judo proposal: replace `cycle_key()` with the single assignment `request.session[HASH_SESSION_KEY] = new_hash`. That already marks the session modified and makes it persist. If the session key must be rotated for security reasons, state that in a comment and in the tests. The test currently pins the unexplained behaviour with `assertNotEqual(request.session.session_key, prev_session_key)`.

## Finding 5: tests cover only the happy path

Evidence: `tests/auth_tests/test_basic.py` adds `test_get_user_fallback_secret`.

Problems:
- No negative test: a session hash that matches neither the current secret nor any fallback must still flush the session and return `AnonymousUser`. That is the security-critical branch of the new code, and it is not exercised with `SECRET_KEY_FALLBACKS` set.
- No test for an empty or missing session hash with fallbacks configured. This is the `if not session_hash` branch that the change introduced.
- No test for a user object that defines only `get_session_auth_hash` (see Finding 1), or for a model that overrides it.
- The "remove the fallback" step only asserts that the user is still returned. It never asserts that `HASH_SESSION_KEY` was actually rewritten with the new secret. The step would pass even if the rewrite were deleted, since the session would still be accepted. The comment "The session hash should be updated" describes an assertion that does not exist.
- The test reads `settings.SECRET_KEY` as a module-level import (`from django.conf import settings` was added), so it depends on the ambient test secret rather than on explicit values.

Remedy: add three small tests (no match, empty hash, rewritten hash value). The test for the rewritten hash value should compare `request.session[HASH_SESSION_KEY]` with `user.get_session_auth_hash()` under the new secret.

## Minor notes (not blocking)

- `docs/ref/contrib/auth.txt` uses a backslash-continued `:meth:` role inside a paragraph. It works, but it is awkward to read and edit.
- The release note and the docs frame the change as a 4.1.8 bugfix, while it adds a public method. That tension is a symptom of Finding 3.

## Verdict for this subsystem

Behaviour for the happy path is right, and the tests that exist pass. The structure is not yet at the approval bar. Findings 1 and 4 are behavioural risks that came from the structural choices, and Findings 2 and 3 are avoidable complexity with a clear simpler alternative.
