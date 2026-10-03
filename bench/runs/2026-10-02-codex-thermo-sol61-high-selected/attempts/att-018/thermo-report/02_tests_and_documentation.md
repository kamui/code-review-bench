# Tests and documentation

## Scope

This subsystem covers `tests/auth_tests/test_basic.py`, `docs/ref/contrib/auth.txt`, `docs/topics/auth/customizing.txt`, and `docs/releases/4.1.8.txt`. Existing middleware tests, session tests, and the default-authentication documentation were read as supporting source evidence.

## Measurements

The complete pinned diff changes six files, adding 69 lines and removing 7. File lengths before and after are:

| File | Base | Head |
| --- | ---: | ---: |
| django/contrib/auth/__init__.py | 230 | 244 |
| django/contrib/auth/base_user.py | 158 | 167 |
| docs/ref/contrib/auth.txt | 704 | 711 |
| docs/releases/4.1.8.txt | 12 | 13 |
| docs/topics/auth/customizing.txt | 1199 | 1206 |
| tests/auth_tests/test_basic.py | 140 | 164 |

These numbers come from `git diff --numstat main...review-head`, `wc -l` on the head files, and the corresponding line deltas; the already-large customization document was also checked with `git show main:docs/topics/auth/customizing.txt | wc -l`. No file crosses 1,000 lines. The existing size of the customization reference does not justify splitting documentation as part of this small bug fix.

## Executed verification

Tests used the provisioned virtualenv, SQLite settings, and no network. Bytecode writing was disabled to keep the checkout unchanged. Each selection was executed once with its flag set, from the clone root, and completed well within five minutes.

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-018/clone /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-018/clone-cache/venv/bin/python tests/runtests.py auth_tests.test_basic auth_tests.test_views auth_tests.test_tokens --settings=test_sqlite
```

The runner reported 137 tests, no system-check issues, and `OK`; exit status was 0. This includes the new fallback-secret regression case and existing login, password-change, and password-reset behavior.

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-018/clone /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-018/clone-cache/venv/bin/python tests/runtests.py auth_tests.test_middleware sessions_tests --settings=test_sqlite
```

The runner reported 381 tests, no system-check issues, and `OK (skipped=2, expected failures=1)`; exit status was 0. Existing `TestAuthenticationMiddleware.test_changed_password_invalidates_session` covers ordinary User password invalidation, and the session suite covers session backends and key cycling.

```sh
git diff --check main...review-head
git status --porcelain
git rev-parse HEAD main review-head
```

The diff whitespace check passed. The checkout was clean at initial inspection, with HEAD and review-head at the pinned head and main at the pinned base. Final checkout verification is recorded in the separate completion note.

No full Django suite or documentation build was run. No new compatibility probe was executed: review findings distinguish static verification from runtime results.

## What the supplied regression proves

`tests/auth_tests/test_basic.py:143–164` logs in under the old secret, retrieves the user under a new secret with the old one configured as a fallback, and asserts that the session key changes. It then retrieves the user under the new secret without the temporary fallback override. With the test settings' normal empty fallback list, that second retrieval protects the authentication-hash renewal: leaving the old hash in the live session would make authentication fail.

The test uses the built-in User and retains the same SessionStore object. It does not prove compatibility with a user implementing the preexisting hash capability independently. It also does not independently test response-cookie persistence or old-session replay. Those are coverage limits, not additional actionable findings established by this review.

## Regression test for the actionable finding

Add one focused `TestGetUser` test with `SECRET_KEY_FALLBACKS=[]`. Supply a backend user with `get_session_auth_hash()` and explicitly no `get_session_auth_fallback_hash()`. An independently defined lightweight user returned through a patched backend is sufficient for testing this capability boundary; simply using an AbstractBaseUser subclass would inherit the new method and miss the defect.

A worked test shape is:

```python
class LegacyHashUser:
    def get_session_auth_hash(self):
        return "current-hash"

request = HttpRequest()
request.session = self.client.session
request.session[SESSION_KEY] = "1"
request.session[BACKEND_SESSION_KEY] = "django.contrib.auth.backends.ModelBackend"
request.session[HASH_SESSION_KEY] = "stale-hash"

with override_settings(
    AUTHENTICATION_BACKENDS=["django.contrib.auth.backends.ModelBackend"],
    SECRET_KEY_FALLBACKS=[],
):
    with mock.patch("django.contrib.auth.load_backend") as load_backend:
        load_backend.return_value.get_user.return_value = LegacyHashUser()
        user = get_user(request)

self.assertIsInstance(user, AnonymousUser)
self.assertEqual(dict(request.session.items()), {})
self.assertIsNone(request.session.session_key)
```

This is an unexecuted proposal, requiring imports for `mock` and the three session-key constants. It uses the default User model's integer primary-key conversion and a real test-client SessionStore, so it focuses the substitute object on the user capability being reviewed. At the pinned head, the fallback call raises before the assertions; with the presence guard, the existing flush behavior supplies the expected result. This test protects actual backward compatibility rather than merely mirroring the fallback loop.

## Documentation assessment

`docs/ref/contrib/auth.txt:698–701` explains fallback verification and links to the new method. `docs/topics/auth/customizing.txt:725–730` defines that method as yielding password HMACs using the fallback settings. Both use the 4.1.8 introduction markers consistent with the release-note target. The release note describes the session-invalidation bug and links to ticket 34384.

These additions do not withdraw the separately documented independent-hash implementation contract in `docs/topics/auth/default.txt:920–927`. Treating the new method as optional in orchestration keeps the documents consistent and avoids forcing a breaking API expansion into a bug-fix release.

The model hash helper, lazy fallback generator, and method reference are appropriate layers for the feature. I found no separate actionable documentation, abstraction, or test-design defect in the changed files. The one summary finding is supported here through the documented contract and the uncovered compatibility path.

