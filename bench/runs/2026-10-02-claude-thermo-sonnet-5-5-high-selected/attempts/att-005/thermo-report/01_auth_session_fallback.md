# Detail 01: django.contrib.auth session verification with SECRET_KEY_FALLBACKS

Scope: `django/contrib/auth/__init__.py` (`get_user`), `django/contrib/auth/base_user.py` (`AbstractBaseUser`), docs, and `tests/auth_tests/test_basic.py`. Range `9b22457987..2396933ca9`, read with `git diff main...review-head`.

## Measurements and commands

- `wc -l django/contrib/auth/__init__.py` gives 244 lines, so there is no 1k-line concern.
- `PYTHONPATH=. venv/bin/python tests/runtests.py auth_tests.test_basic --settings=test_sqlite` finished with `OK`, so the new test passes.
- `grep -rn "get_session_auth_hash\|HASH_SESSION_KEY" django` shows three consumers of the session auth hash: `login()` (lines 103-140), `get_user()` (200-218), and `update_session_auth_hash()` (243-244). Only `get_user()` was taught about fallbacks.
- `django/contrib/auth/tokens.py` and `django/core/signing.py` already handle `SECRET_KEY_FALLBACKS` by holding the fallback list in a signer/generator and looping inside it.

## Finding 1: get_user() now calls an API that its own guard does not protect (correctness and boundary)

Status: verified by reading the code. I did not run a repro.

`get_user()` guards the whole verification block with `hasattr(user, "get_session_auth_hash")`. That guard exists because user objects need not inherit `AbstractBaseUser`; the duck-typed method is the contract. The patch adds an unconditional call to `user.get_session_auth_fallback_hash()` inside that block. The new method exists only on `AbstractBaseUser`. A user class that defines `get_session_auth_hash` itself but does not inherit `AbstractBaseUser` will pass the guard. Whenever the current-secret hash fails to match and the session has a hash, it raises `AttributeError` from `get_user()` instead of flushing the session. That path is hit on every password change or invalid session. The old code logged such sessions out cleanly. The patch also adds a second duck-typed method to the implicit custom-user contract and documents it in `customizing.txt` as part of the `AbstractBaseUser` API.

A related problem exists for subclasses that override `get_session_auth_hash()`, which is documented and common. The fallback generator calls `_get_session_auth_hash` directly and never goes through the override. An overriding subclass therefore keeps its sessions invalidated on key rotation, which is the bug this PR claims to fix, and nothing signals that. The override and the fallback generator have to be kept in sync by hand.

Remedy: keep the verification logic behind one canonical method. See the code-judo proposal below.

## Finding 2: get_user() grows a deeper, more tangled conditional ladder (spaghetti)

Status: verified by reading.

The original verification was a three-line expression. The patch turns it into a nested if/else that conditionally defines `session_auth_hash`, followed by a second condition and a generator-expression `any(...)`. It ends in a three-way outcome (verified, verified-by-fallback, flush). Several smells follow.

- `session_auth_hash` is bound only inside the `else` branch, then read later inside the fallback branch. This is correct only because `session_hash` is truthy in both places, a hidden coupling between two separate conditions.
- The `session_hash` emptiness check is done twice, once in the first `if not session_hash` and again in `if session_hash and any(...)`.
- `session_hash_verified` is now a flag with a single use and no other purpose.
- Session mutation (`cycle_key()` plus rewriting `HASH_SESSION_KEY`) is added inside a function that is otherwise a read path. The ladder is already nested six levels deep, with `try/else/if/if/if/if`.

The same loop-the-fallbacks idea is already expressed once for tokens and signing, inside the helper that owns the secret. Here it is scattered across the caller instead.

## Finding 3: Redundant helper layering in base_user.py

Status: verified by reading.

`get_session_auth_hash()` is reduced to `return self._get_session_auth_hash()`, a pass-through wrapper whose only job is to supply `secret=None`. The private `_get_session_auth_hash(secret=None)` is the real implementation. A third public method, `get_session_auth_fallback_hash()`, is a generator that loops over settings. That is three methods for one HMAC. A smaller shape is a single `get_session_auth_hash(self, secret=None)`, which keeps the existing public method working with an added optional argument, but that changes the signature for overriders. The cleaner fix is not to add a public generator at all (see the judo move).

The new generator reads `settings` in `base_user.py`, so a model module now imports `django.conf.settings` solely to iterate a list. The other fallback consumers keep that knowledge in the signer.

## Finding 4: Verification policy is split across three call sites (missed canonical home)

Status: verified by reading.

`login()` still compares the stored hash against the current-secret hash only and flushes on mismatch. `update_session_auth_hash()` writes the current hash. `get_user()` now has the fallback rule. The rule "a stored hash is valid if it matches current or any fallback hash" exists in only one of the three places that compare hashes. A user who still holds a fallback-hashed session and calls `login()` for the same user before `get_user()` has run will have the session flushed. That is an edge case, but it shows the policy has no single home.

## Finding 5: Test coverage is thin for the branches added (maintainability)

Status: verified by reading and by the passing run.

The single test covers only the happy path. Branches left untested are an invalid hash with fallbacks configured (must still flush), a missing session hash, and a non-`AbstractBaseUser` user. The second half of the test, which checks the upgraded hash by dropping the fallback, asserts only that the user is returned. It does not assert that `HASH_SESSION_KEY` was rewritten, and `cycle_key()` is only indirectly observed. The 4.1.8 release note also names the bug without noting that `get_session_auth_fallback_hash` is new public API.

## Worked code-judo proposal

Move the entire rule onto one place that owns the secret list, and make `get_user()` stay a flat read path.

1. In `AbstractBaseUser`, keep `get_session_auth_hash(self)` unchanged in signature and add one private helper that takes a secret. Add a single public-or-private predicate, for example `_verify_session_auth_hash(self, session_hash)`. It would use `constant_time_compare` against the current hash and then against each fallback secret. It would return `(verified, needs_upgrade)`, or the matching secret. Because it calls `self.get_session_auth_hash()` for the current key, overrides are respected, and fallbacks can reuse the same `salted_hmac` call through a `secret=` argument.
2. In `get_user()` the block collapses to roughly:

```python
if hasattr(user, "get_session_auth_hash"):
    session_hash = request.session.get(HASH_SESSION_KEY)
    verified = _verify_session_hash(user, session_hash)   # module-level, uses getattr for fallbacks
    if verified is None:
        request.session.flush()
        user = None
    elif verified is FALLBACK:
        request.session.cycle_key()
        request.session[HASH_SESSION_KEY] = user.get_session_auth_hash()
```

3. `_verify_session_hash` lives next to `login()` and `update_session_auth_hash()` in `auth/__init__.py`. `login()` can call it as well, so the policy is in one place. It uses `getattr(user, "get_session_auth_fallback_hash", None)` so non-`AbstractBaseUser` user classes are not broken.

This deletes the flag variable, the duplicate emptiness check, the conditionally-bound local, and the pass-through wrapper, while fixing Findings 1 and 4. No behavior changes for current-secret sessions.

## Verification status summary

| Finding | Basis | Verified |
| --- | --- | --- |
| 1 (AttributeError, override bypass) | Code reading | Yes, by reading; not executed |
| 2 (ladder) | Code reading | Yes |
| 3 (wrapper layering) | Code reading | Yes |
| 4 (policy split) | Code reading | Yes |
| 5 (tests) | Code reading, test run | Yes |
