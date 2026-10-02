# 03 — Tests and documentation (F7, plus doc gaps for F1 and F2)

Subsystem: `tests/auth_tests/test_basic.py`, `docs/ref/contrib/auth.txt`, `docs/topics/auth/customizing.txt`, `docs/topics/auth/default.txt`, `docs/releases/4.1.8.txt`.

## F7 — test coverage

### What the PR added

`tests/auth_tests/test_basic.py` lines 143–164, one test:

```python
def test_get_user_fallback_secret(self):
    created_user = User.objects.create_user("testuser", "test@example.com", "testpw")
    self.client.login(username="testuser", password="testpw")
    request = HttpRequest()
    request.session = self.client.session
    prev_session_key = request.session.session_key
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

It passes at head (`auth_tests.test_basic`: 13 tests, OK).

### What it does not pin

- **Negative case with fallbacks configured.** No test asserts that a session hash matching neither `SECRET_KEY` nor any entry of `SECRET_KEY_FALLBACKS` is flushed and yields `AnonymousUser`. This is the security-relevant direction: a mistake in the `any(...)` expression that made it too permissive would not be caught.
- **A fallback other than the first.** Only a single-element `SECRET_KEY_FALLBACKS` is exercised.
- **Persistence of the upgrade.** The second block reuses the same in-memory `request.session`, so it proves `request.session[HASH_SESSION_KEY]` was assigned, not that the session was saved. A fresh `SessionStore(request.session.session_key)` after `request.session.save()` would prove the latter.
- **User models without the new method.** Nothing covers a user object that has `get_session_auth_hash()` but not `get_session_auth_fallback_hash()` (F1).
- **The model method itself.** There is no unit test of `AbstractBaseUser.get_session_auth_fallback_hash()` — for example that it yields nothing when `SECRET_KEY_FALLBACKS` is empty and one hash per entry otherwise, each equal to `get_session_auth_hash()` under that secret.

The assertion on line 158 also pins the `cycle_key()` behaviour questioned in F3; if that call is removed the assertion goes with it.

### Suggested additions

```python
def test_get_user_fallback_secret_no_match(self):
    User.objects.create_user("testuser", "test@example.com", "testpw")
    self.client.login(username="testuser", password="testpw")
    request = HttpRequest()
    request.session = self.client.session
    with override_settings(
        SECRET_KEY="newsecret", SECRET_KEY_FALLBACKS=["someothersecret"]
    ):
        self.assertIsInstance(get_user(request), AnonymousUser)
        self.assertNotIn(HASH_SESSION_KEY, request.session)
```

and, in the model tests:

```python
@override_settings(SECRET_KEY="new", SECRET_KEY_FALLBACKS=["old1", "old2"])
def test_session_auth_fallback_hash(self):
    user = User(password="x")
    with override_settings(SECRET_KEY="old1"):
        old1 = user.get_session_auth_hash()
    with override_settings(SECRET_KEY="old2"):
        old2 = user.get_session_auth_hash()
    self.assertEqual(list(user.get_session_auth_fallback_hash()), [old1, old2])
```

These were not run; they are sketches for the author.

## Documentation gaps that accompany F1 and F2

- `docs/topics/auth/default.txt` lines 919–926 still describe the custom-model contract as "implements its own `get_session_auth_hash()` method". After this PR a second method participates in verification. Either the code treats it as optional (the F1 remedy) and the section says so, or the section must say it is required.
- `docs/topics/auth/customizing.txt` lines 725–730 describe the new method as "Yields the HMAC of the password field using `SECRET_KEY_FALLBACKS`" and stop there. It should say that a model overriding `get_session_auth_hash()` must keep the two consistent (F2), unless the implementation is restructured so that this is automatic.
- `docs/ref/contrib/auth.txt` lines 698–701 describe the fallback verification but not the side effect: on a fallback match the stored hash is rewritten and, as written, the session key is cycled. If the `cycle_key()` call stays (F3), that belongs here.
- `docs/releases/4.1.8.txt` records the bug fix but not the new public method on `AbstractBaseUser`. A new documented method arriving in a patch release is unusual enough to deserve a clause.

## Commands

```text
grep -n "" tests/auth_tests/test_basic.py | sed -n 141,164p
sed -n 915,966p docs/topics/auth/default.txt
grep -n "get_session_auth_fallback_hash" -B3 -A6 docs/topics/auth/customizing.txt
PYTHONPATH=$PWD ../clone-cache/venv/bin/python tests/runtests.py auth_tests.test_basic --settings=test_sqlite
```
