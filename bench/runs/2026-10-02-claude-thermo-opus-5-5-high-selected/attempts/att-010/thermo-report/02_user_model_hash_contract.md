# 02 — `AbstractBaseUser` session-hash contract (F2, F5)

Subsystem: `django/contrib/auth/base_user.py`, with `django/utils/crypto.py` and `docs/topics/auth/customizing.txt`.

## What the diff did

Before:

```python
def get_session_auth_hash(self):
    """
    Return an HMAC of the password field.
    """
    key_salt = "django.contrib.auth.models.AbstractBaseUser.get_session_auth_hash"
    return salted_hmac(
        key_salt,
        self.password,
        algorithm="sha256",
    ).hexdigest()
```

After (lines 135–153):

```python
def get_session_auth_hash(self):                                   # 135
    """
    Return an HMAC of the password field.
    """
    return self._get_session_auth_hash()

def get_session_auth_fallback_hash(self):                          # 141
    for fallback_secret in settings.SECRET_KEY_FALLBACKS:
        yield self._get_session_auth_hash(secret=fallback_secret)

def _get_session_auth_hash(self, secret=None):                     # 145
    key_salt = "django.contrib.auth.models.AbstractBaseUser.get_session_auth_hash"
    return salted_hmac(
        key_salt,
        self.password,
        secret=secret,
        algorithm="sha256",
    ).hexdigest()
```

Plus a new `from django.conf import settings` at the top of the module. File length at head: 167 lines.

## F2 — the fallback path does not go through the overridable method

### Evidence

`get_session_auth_hash()` is public, documented API (`docs/topics/auth/customizing.txt` line 720) and the session docs describe a model that "implements its own `get_session_auth_hash()` method" (`docs/topics/auth/default.txt` lines 919–921). Overriding it on an `AbstractBaseUser` subclass is the ordinary way to bind sessions to something other than, or in addition to, the password field.

`get_session_auth_fallback_hash()` at line 143 calls `self._get_session_auth_hash(secret=...)`, the private base implementation, not anything the subclass overrode.

### Failure scenario

```python
class User(AbstractBaseUser):
    def get_session_auth_hash(self):
        return salted_hmac(
            "myapp.session", f"{self.password}{self.token_version}", algorithm="sha256"
        ).hexdigest()
```

1. Sessions store the override's output under the old secret.
2. Operator rotates `SECRET_KEY`, moving the old value to `SECRET_KEY_FALLBACKS`.
3. `get_user()` computes the override under the new secret: mismatch. It then iterates `get_session_auth_fallback_hash()`, which yields the stock `salted_hmac("django.contrib.auth.models.AbstractBaseUser.get_session_auth_hash", self.password, secret=old)`. That has a different salt and a different message from what the override produced, so it cannot match.
4. Session flushed. The user is logged out, which is the bug the PR's release note says is fixed. No error, no warning.

Unlike F1 this does not crash, which makes it worse to diagnose: the operator followed the documented rotation procedure and has no signal about why it did not work.

### Verification status

Code trace only; not executed.

### Remedy

The two public methods must be derived from one overridable implementation. Either of these works:

- Document the secret-parameterised function as the override point, and define both public methods in terms of it (worked shape below).
- Keep the PR's shape but state in `customizing.txt`, next to both method entries, that a model overriding `get_session_auth_hash()` must also override `get_session_auth_fallback_hash()` to produce the same hash under each of `SECRET_KEY_FALLBACKS`.

The first deletes a way to get it wrong; the second only documents it.

## F5 — method trio, optional parameter, singular name for a generator

### Evidence

- `get_session_auth_hash()` is now a one-line pass-through to `_get_session_auth_hash()`. The docstring stayed on the wrapper; the function that does the work has none.
- `get_session_auth_fallback_hash()` is public and documented (`customizing.txt` lines 725–730) but has no docstring in the code, and its name is singular while it is a generator yielding `len(SECRET_KEY_FALLBACKS)` values. The docs entry had to use the verb "Yields" to paper over this.
- A generator is an awkward public return type for a model method: it is always truthy, single-use, and cannot be compared or indexed. The only caller wraps it in `any()`, where a list would behave identically.
- `secret=None` on the private helper means "use `settings.SECRET_KEY`", a default inherited from `salted_hmac` (`django/utils/crypto.py` lines 18–27). That is fine, but it is the third place the "current secret" default is spelled.
- `base_user.py` now imports `django.conf.settings` purely so that a model method can enumerate a rotation setting. Elsewhere in this package the iteration over fallbacks lives with the verifier, not with the thing being hashed: `PasswordResetTokenGenerator.check_token()` does `for secret in [self.secret, *self.secret_fallbacks]` in `tokens.py` line 69.

### Worked proposal

One real implementation, which is also the documented override point, and two thin public entry points whose relationship is obvious:

```python
def get_session_auth_hash(self):
    """
    Return an HMAC of the password field.
    """
    return self._get_session_auth_hash()

def get_session_auth_fallback_hashes(self):
    """
    Return the session auth hash computed with each of SECRET_KEY_FALLBACKS.
    """
    return [
        self._get_session_auth_hash(secret=fallback_secret)
        for fallback_secret in settings.SECRET_KEY_FALLBACKS
    ]

def _get_session_auth_hash(self, secret=None):
    """
    Return an HMAC of the password field keyed with the given secret
    (SECRET_KEY by default). Override this to customize the hash; both
    get_session_auth_hash() and get_session_auth_fallback_hashes() use it.
    """
    key_salt = "django.contrib.auth.models.AbstractBaseUser.get_session_auth_hash"
    return salted_hmac(
        key_salt,
        self.password,
        secret=secret,
        algorithm="sha256",
    ).hexdigest()
```

That keeps the PR's three-method count but makes each one earn its place: the private method is the single source of truth and says so, and the plural method has a plural name, a concrete return type and a docstring.

If the project would rather not bless an underscore method as an override point, the more ambitious move is to delete the model-level fallback method entirely and have the verifier own the iteration, passing the secret in:

```python
# django/contrib/auth/__init__.py
def _session_auth_hashes(user):
    yield user.get_session_auth_hash()
    make_hash = getattr(user, "_get_session_auth_hash", None)
    if make_hash is not None:
        for secret in settings.SECRET_KEY_FALLBACKS:
            yield make_hash(secret=secret)
```

That removes the `settings` import from `base_user.py`, removes one public method from the user-model API, and matches the `[secret, *fallbacks]` loop already used by `tokens.py` and `signing.py`. It trades a public method for a private hook, so it is a maintainer call; either shape is better than two independently overridable public methods that must agree but share no code path visible to the subclass author.

## Commands

```text
sed -n 120,160p django/contrib/auth/base_user.py
sed -n 18,40p django/utils/crypto.py
grep -n "get_session_auth_fallback_hash" -B3 -A6 docs/topics/auth/customizing.txt
sed -n 915,966p docs/topics/auth/default.txt
```
