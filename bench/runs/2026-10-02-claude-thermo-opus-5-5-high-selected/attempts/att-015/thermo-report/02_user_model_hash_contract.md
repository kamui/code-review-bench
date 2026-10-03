# 02 — User model hash contract, docs and tests

Covers F5 and F6 from `summary.md`. Probe setup and raw output are in
`01_session_verification_flow.md` under "How the probes were run".

## The code under review

`django/contrib/auth/base_user.py:135-152` on `review-head`:

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

Measurements: one method on `main` becomes three; the file goes from 158 to 167
lines and gains an import of `django.conf.settings`.

## F5 — parallel hook that bypasses overrides

**Status: reproduced on `review-head` (P2). Not a regression: on `main` these
projects also lose sessions on rotation. The finding is that the fix does not
reach them and does not say so.**

### What goes wrong

`get_session_auth_hash()` is public and documented
(`docs/topics/auth/customizing.txt:720-723`), and
`docs/topics/auth/default.txt:920-927` describes it as the thing a project
implements to control session invalidation. The new fallback generator does not
go through it. It calls the private `_get_session_auth_hash()` directly.

So for a subclass that overrides `get_session_auth_hash()`:

- the stored hash was produced by the override under the old key;
- the fallback candidates are produced by the base formula under the old key;
- they cannot match, and the session is flushed.

Probe P2 models this with a real `User` whose `get_session_auth_hash` returns
`"custom:" + <base hash>`:

```text
P2 RESULT: after rotation -> AnonymousUser
```

The session verified fine before rotation (the probe asserts that first), and
is rejected after rotation even though the old key is in
`SECRET_KEY_FALLBACKS`.

### Why the split is partly forced

The clean design would be one hook parameterised by secret. That is not
available: `get_session_auth_hash(self)` is overridden in third-party code with
that exact signature, so the framework cannot start passing `secret=`. A
private helper plus a second public method is a reasonable response to that
constraint.

What is avoidable:

1. **The coupling is undocumented.** The new docs entry at
   `docs/topics/auth/customizing.txt:725-730` reads "Yields the HMAC of the
   password field using `SECRET_KEY_FALLBACKS`. Used by `get_user()`." It should
   say that a project overriding `get_session_auth_hash()` must override this
   method to match, or its sessions will not survive key rotation.
2. **No docstring.** `get_session_auth_hash()` has one;
   `get_session_auth_fallback_hash()` and `_get_session_auth_hash()` do not.
   The fact that the public method is a generator is only discoverable from the
   body.
3. **The name is singular.** The method yields one hash per fallback key. A
   caller reading `user.get_session_auth_fallback_hash()` expects a hash, not an
   iterator; comparing its return value directly would be a silent bug, since a
   generator object is never equal to a string. This is new public API in a
   patch release, so the name is expensive to change later.
4. **`get_session_auth_hash()` is now a pure pass-through.** It is harmless, but
   it is one more hop for a reader. It is tolerable only because the public
   signature cannot change; the docstring of the private method should say that
   it is the single place the HMAC is defined.

### Suggested shape

Keeping the PR's structure and fixing the contract:

```python
    def get_session_auth_hash(self):
        """
        Return an HMAC of the password field.
        """
        return self._get_session_auth_hash()

    def get_session_auth_fallback_hash(self):
        """
        Yield the session auth hash computed with each of
        SECRET_KEY_FALLBACKS. Override together with get_session_auth_hash().
        """
        for fallback_secret in settings.SECRET_KEY_FALLBACKS:
            yield self._get_session_auth_hash(secret=fallback_secret)
```

Together with the `getattr` lookup proposed under F1, a project that overrides
only the first method keeps today's behaviour, which is the safe default.

### Related docs observations

- `docs/ref/contrib/auth.txt:698-701` describes the new behaviour of
  `get_user()` accurately. If `cycle_key()` stays (F2), this paragraph should
  mention that the session key changes; if it goes, nothing needs adding.
- `docs/topics/auth/default.txt:958-964` already tells readers to use
  `SECRET_KEY_FALLBACKS` to avoid invalidating sessions. That note becomes true
  with this PR for the default hash only; the caveat for overrides belongs
  there or in the `customizing.txt` entry.

## F6 — test coverage

**Status: verified by reading and by running the module (13 tests, OK).**

`tests/auth_tests/test_basic.py:143-164` adds one test:

```python
    def test_get_user_fallback_secret(self):
        ...
        with override_settings(
            SECRET_KEY="newsecret",
            SECRET_KEY_FALLBACKS=[settings.SECRET_KEY],
        ):
            user = get_user(request)
            self.assertIsInstance(user, User)
            self.assertEqual(user.username, created_user.username)
            self.assertNotEqual(request.session.session_key, prev_session_key)
        # Remove the fallback secret.
        # The session hash should be updated using the current secret.
        with override_settings(SECRET_KEY="newsecret"):
            user = get_user(request)
            self.assertIsInstance(user, User)
            self.assertEqual(user.username, created_user.username)
```

What it establishes: a session signed with the old key is accepted, and the
stored hash is replaced with the new-key hash.

What is not exercised by this PR's tests:

| Case | Why it matters |
| --- | --- |
| Fallback list present but not containing the signing key | the only path where the generator is fully consumed and the session must still be flushed |
| User object with `get_session_auth_hash()` and no fallback hook | F1; a test here would have caught the `AttributeError` |
| Same-user `login()` after rotation | F4 |
| Second request with the old cookie after the first one re-signed | F2 |

The existing password-change tests cover the flush branch only with an empty
fallback list.

One assertion deserves a second look:
`assertNotEqual(request.session.session_key, prev_session_key)` turns
`cycle_key()` into tested behaviour without recording a reason. If F2 is
accepted, this line goes away with the call. If `cycle_key()` is kept, a comment
stating the security purpose should sit next to the assertion.

A smaller point: the second `with` block relies on `SECRET_KEY_FALLBACKS`
defaulting to `[]` in the test settings. Passing `SECRET_KEY_FALLBACKS=[]`
explicitly would make "the fallback has been removed" visible in the test
rather than implied by the comment.
