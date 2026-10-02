# 02 — User-model API, documentation and tests

Scope: `django/contrib/auth/base_user.py:135-152`,
`docs/topics/auth/customizing.txt:725-730`,
`docs/topics/auth/default.txt:958-964`, and
`tests/auth_tests/test_basic.py:143-164`.

## Finding E — the override point and the fallback path compute different things

Evidence (head):

```python
def get_session_auth_hash(self):
    """
    Return an HMAC of the password field.
    """
    return self._get_session_auth_hash()

def get_session_auth_fallback_hash(self):
    for fallback_secret in settings.SECRET_KEY_FALLBACKS:
        yield self._get_session_auth_hash(secret=fallback_secret)

def _get_session_auth_hash(self, secret=None):
    key_salt = "django.contrib.auth.models.AbstractBaseUser.get_session_auth_hash"
    return salted_hmac(
        key_salt,
        self.password,
        secret=secret,
        algorithm="sha256",
    ).hexdigest()
```

One concept — "the session hash for this user under a given secret" — is now
spread over three methods. `get_session_auth_hash()` is the documented
override point (`docs/topics/auth/customizing.txt:720-723`), and projects
override it to mix in more than the password. After this PR the fallback path
does not go through that override: it goes through the new private
`_get_session_auth_hash()`. A subclass that overrides only the public method
keeps working for the current key, but its fallback hashes are computed with
the stock formula, never match what `login()` stored, and the session is
flushed. The ticket's bug survives for precisely the projects that customised
the hash, and nothing fails loudly to tell them.

The new documentation does not warn about this. `customizing.txt:725-730` says
only that the method "Yields the HMAC of the password field using
`SECRET_KEY_FALLBACKS`", and the note at `docs/topics/auth/default.txt:958-964`
that already ties `get_session_auth_hash()` to `SECRET_KEY_FALLBACKS` was left
untouched.

Two smaller points belong to the same remedy. The method is named in the
singular (`..._fallback_hash`) but is a generator of zero or more hashes, so
the name misdescribes the return shape that the caller must iterate. And it is
a new public, documented method with no docstring, sitting directly beneath a
sibling that has one.

Verification status: read from source and docs; not executed.

Remedy. Make the three methods compose in one direction and say so. The
smallest change is to state in both doc locations that a model overriding
`get_session_auth_hash()` must override the fallback method consistently, give
the method a docstring, and name it for what it returns. A stronger version
makes `_get_session_auth_hash(secret=None)` the single place the formula
lives and documents *that* as the thing to override, so the public pair can
never drift apart:

```python
def get_session_auth_hash(self):
    """Return an HMAC of the password field."""
    return self._get_session_auth_hash()

def get_session_auth_fallback_hashes(self):
    """Yield the session auth hash for each of SECRET_KEY_FALLBACKS."""
    for fallback_secret in settings.SECRET_KEY_FALLBACKS:
        yield self._get_session_auth_hash(secret=fallback_secret)
```

Either way the contract needs to be written down; today it is implicit.

## Finding F — the test covers one of the four outcomes

Evidence. `test_get_user_fallback_secret` (`tests/auth_tests/test_basic.py:143-164`)
logs in under the default key, switches to `SECRET_KEY="newsecret"` with the
old key as the only fallback, asserts the user is returned and the session key
changed, then drops the fallback and asserts the user is still returned.

That is the happy path only. The branch this PR adds is security-sensitive —
it widens the set of hashes that authenticate a session — and the cases that
show the widening is bounded are not exercised:

- fallbacks configured but none of them match (the session must be flushed and
  `AnonymousUser` returned — this is the path that guards against the `any()`
  being accidentally permissive);
- a stale hash after a password change while fallbacks are configured;
- more than one fallback, with the match not in first position;
- a user object that defines `get_session_auth_hash` but not the fallback
  method (Finding A in `01_session_verification_flow.md`).

The one assertion that goes beyond "the user came back" is
`assertNotEqual(request.session.session_key, prev_session_key)`, which pins the
`cycle_key()` side effect questioned in Finding C rather than the property the
ticket cares about. The property that matters — the stored hash is rewritten
under the new key — is only checked indirectly by the second `get_user()` call.

Verification status: `auth_tests.test_basic` was run once at the head SHA with
`--settings=test_sqlite`: 13 tests, OK. The gaps above were established by
reading the test and by `grep -rn "get_session_auth_hash\|session_auth" tests`,
which returns no other coverage.

Remedy. Add the negative case (wrong fallback → `AnonymousUser`, session
flushed) and the non-`AbstractBaseUser` case alongside the existing test, and
assert directly on `request.session[HASH_SESSION_KEY]` after the upgrade.
