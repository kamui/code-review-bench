You are the independent validation gate for the code-review findings below. Evaluate every finding separately under fresh inspection and under the shared protected-subject policy below. Outside a protected subject, false positives are common; reject a finding when the cited code does not prove it, it predates and is unaffected by this diff, surrounding code handles it, or it is only an unsupported preference. Inside a protected subject, a rejection requires one of the evidence forms the policy names.

Do not let one finding's outcome influence another. Do not invent new findings.

<findings-to-validate>
[
 {
  "#": 1,
  "severity": "P1",
  "title": "get_user() raises AttributeError for user models lacking get_session_auth_fallback_hash",
  "file": "django/contrib/auth/__init__.py",
  "line": 215,
  "confidence": 100,
  "autofix_class": "gated_auto",
  "owner": "downstream-resolver",
  "requires_verification": true,
  "pre_existing": false,
  "reviewers": [
   "correctness",
   "api-contract",
   "adversarial",
   "testing",
   "security",
   "fast-pass"
  ],
  "first_evidence": "django/contrib/auth/__init__.py:215 -- for fallback_auth_hash in user.get_session_auth_fallback_hash()",
  "why_it_matters": "Django documents that a user model may either inherit AbstractBaseUser or just implement its own get_session_auth_hash(), and get_user() still guards only on hasattr(user, \"get_session_auth_hash\"). For such duck-typed users, any session whose stored hash no longer matches (the normal password-changed-elsewhere invalidation path, or key rotation) now raises AttributeError instead of flushing the session. This happens even when SECRET_KEY_FALLBACKS is empty. Because the session is never flushed, every later request on that cookie returns a 500 until the cookie is cleared. That is a regression in a 4.1.8 patch release. Reproduced with a standalone script: a user object that has only get_session_auth_hash and a stale session hash -> `AttributeError: 'DuckUser' object has no attribute 'get_session_auth_fallback_hash'`.",
  "evidence": [
   "django/contrib/auth/__init__.py:215 -- for fallback_auth_hash in user.get_session_auth_fallback_hash()",
   "django/contrib/auth/__init__.py:200 -- if hasattr(user, \"get_session_auth_hash\"):  (only guard; fallback method assumed)",
   "docs/topics/auth/default.txt:920-923 -- If your AUTH_USER_MODEL inherits from AbstractBaseUser or implements its own get_session_auth_hash() method, authenticated sessions will include the hash returned by this function.",
   "Standalone repro (tmp/correctness/dk.py): get_user() with a user object that has only get_session_auth_hash and a stale session hash -> RAISED AttributeError 'DuckUser' object has no attribute 'get_session_auth_fallback_hash'",
   "django/contrib/auth/__init__.py:200 -- if hasattr(user, \"get_session_auth_hash\"):  (only the old method is feature-detected)",
   "docs/topics/auth/default.txt:920-923 -- 'If your AUTH_USER_MODEL inherits from AbstractBaseUser or implements its own get_session_auth_hash() method, authenticated sessions will include the hash returned by this function.'",
   "Reproduced with standalone script (tmp/api-contract/check.py): duck-typed user defining only get_session_auth_hash plus stale session hash -> AttributeError: 'DuckUser' object has no attribute 'get_session_auth_fallback_hash'; before this diff the same input returned AnonymousUser after session.flush().",
   "django/contrib/auth/__init__.py:200 -- if hasattr(user, \"get_session_auth_hash\"):  (only this method is feature-detected)",
   "docs/topics/auth/default.txt:920-922 -- 'If your AUTH_USER_MODEL inherits from AbstractBaseUser or implements its own get_session_auth_hash() method, authenticated sessions will include the hash'",
   "Reproduced with a scratch script (tmp/adversarial/duck.py): a backend returns a plain object with get_session_auth_hash() returning 'hash-v2' and the session stores 'hash-v1'. get_user(request) raises AttributeError: 'DuckUser' object has no attribute 'get_session_auth_fallback_hash'. Before this diff the same input flushed the session and returned AnonymousUser.",
   "The exception fires before request.session.flush(), so the stale session survives. Every later lazy request.user evaluation (AuthenticationMiddleware SimpleLazyObject / auser) raises again.",
   "django/contrib/auth/__init__.py:200 -- if hasattr(user, \"get_session_auth_hash\"):",
   "Scratch repro (tmp/testing/scripts/ducktype.py) against the checkout: class U with only get_session_auth_hash, stale stored hash -> raised: AttributeError 'U' object has no attribute 'get_session_auth_fallback_hash'",
   "django/contrib/auth/__init__.py:200 -- if hasattr(user, \"get_session_auth_hash\"):  (the only capability guard; the new method is called unguarded inside it)",
   "django/contrib/auth/__init__.py:103 -- login() uses the same hasattr(user, \"get_session_auth_hash\") duck-typing guard, which shows that user objects without AbstractBaseUser methods are supported",
   "django/contrib/auth/base_user.py:141-143 -- get_session_auth_fallback_hash is defined only on AbstractBaseUser"
  ],
  "suggested_fix": "In get_user(), look the fallback method up defensively: `fallback_hashes = getattr(user, \"get_session_auth_fallback_hash\", None)`, and only run the `any(constant_time_compare(session_hash, h) for h in fallback_hashes())` check when it is not None; otherwise fall through to `request.session.flush(); user = None` as before. Add a TestGetUser case whose backend returns a non-AbstractBaseUser object defining only get_session_auth_hash() with a stale stored HASH_SESSION_KEY, asserting AnonymousUser is returned and the session is flushed."
 },
 {
  "#": 2,
  "severity": "P1",
  "title": "No test rejects a non-matching hash when fallbacks are set",
  "file": "django/contrib/auth/__init__.py",
  "line": 213,
  "confidence": 75,
  "autofix_class": "gated_auto",
  "owner": "downstream-resolver",
  "requires_verification": true,
  "pre_existing": false,
  "reviewers": [
   "testing"
  ],
  "first_evidence": "django/contrib/auth/__init__.py:213-216 -- if session_hash and any(constant_time_compare(session_hash, fallback_auth_hash) for fallback_auth_hash in user.get_session_auth_fallback_hash()):",
  "why_it_matters": "The only check that stops forged or stale sessions from being accepted once SECRET_KEY_FALLBACKS is set is the fallback hash comparison, and no test exercises it. A mutation that replaced the any(constant_time_compare(...)) check with `list(user.get_session_auth_fallback_hash())`, which accepts any non-empty session hash whenever fallbacks are configured, passed all 984 auth_tests and sessions_tests. Under that mutation, sessions left over from a password change would be silently re-authenticated and their hash rewritten during every key rotation. The existing password-change invalidation tests run with the default empty SECRET_KEY_FALLBACKS, so they never enter this branch. A negative test with fallbacks configured would pin the 'must not accept forged/stale sessions' requirement.",
  "evidence": [
   "django/contrib/auth/__init__.py:213-216 -- if session_hash and any(constant_time_compare(session_hash, fallback_auth_hash) for fallback_auth_hash in user.get_session_auth_fallback_hash()):",
   "tests/auth_tests/test_basic.py:151-154 -- the only test that sets SECRET_KEY_FALLBACKS for get_user uses a matching fallback (positive path only)",
   "Mutation run on scratch copy of 2396933ca9: replaced the any(...) with `list(user.get_session_auth_fallback_hash())`; `runtests.py auth_tests sessions_tests` -> Ran 984 tests, OK"
  ],
  "suggested_fix": "Add TestGetUser cases under override_settings(SECRET_KEY='newsecret', SECRET_KEY_FALLBACKS=[settings.SECRET_KEY]): (1) a stored hash that matches no key gives AnonymousUser and a flushed session; (2) after user.set_password()+save(), get_user returns AnonymousUser. Together these show that a password change still invalidates sessions while fallbacks are active."
 }
]
</findings-to-validate>

<diff>
diff --git a/django/contrib/auth/__init__.py b/django/contrib/auth/__init__.py
index 155330c596..2c81d62a0c 100644
--- a/django/contrib/auth/__init__.py
+++ b/django/contrib/auth/__init__.py
@@ -192,26 +192,40 @@ def get_user(request):
         backend_path = request.session[BACKEND_SESSION_KEY]
     except KeyError:
         pass
     else:
         if backend_path in settings.AUTHENTICATION_BACKENDS:
             backend = load_backend(backend_path)
             user = backend.get_user(user_id)
             # Verify the session
             if hasattr(user, "get_session_auth_hash"):
                 session_hash = request.session.get(HASH_SESSION_KEY)
-                session_hash_verified = session_hash and constant_time_compare(
-                    session_hash, user.get_session_auth_hash()
-                )
+                if not session_hash:
+                    session_hash_verified = False
+                else:
+                    session_auth_hash = user.get_session_auth_hash()
+                    session_hash_verified = constant_time_compare(
+                        session_hash, session_auth_hash
+                    )
                 if not session_hash_verified:
-                    request.session.flush()
-                    user = None
+                    # If the current secret does not verify the session, try
+                    # with the fallback secrets and stop when a matching one is
+                    # found.
+                    if session_hash and any(
+                        constant_time_compare(session_hash, fallback_auth_hash)
+                        for fallback_auth_hash in user.get_session_auth_fallback_hash()
+                    ):
+                        request.session.cycle_key()
+                        request.session[HASH_SESSION_KEY] = session_auth_hash
+                    else:
+                        request.session.flush()
+                        user = None
 
     return user or AnonymousUser()
 
 
 def get_permission_codename(action, opts):
     """
     Return the codename of the permission for the specified action.
     """
     return "%s_%s" % (action, opts.model_name)
 
diff --git a/django/contrib/auth/base_user.py b/django/contrib/auth/base_user.py
index 5ee30bf59c..e205ccccf2 100644
--- a/django/contrib/auth/base_user.py
+++ b/django/contrib/auth/base_user.py
@@ -1,17 +1,18 @@
 """
 This module allows importing AbstractBaseUser even when django.contrib.auth is
 not in INSTALLED_APPS.
 """
 import unicodedata
 import warnings
 
+from django.conf import settings
 from django.contrib.auth import password_validation
 from django.contrib.auth.hashers import (
     check_password,
     is_password_usable,
     make_password,
 )
 from django.db import models
 from django.utils.crypto import get_random_string, salted_hmac
 from django.utils.deprecation import RemovedInDjango51Warning
 from django.utils.translation import gettext_lazy as _
@@ -128,24 +129,32 @@ class AbstractBaseUser(models.Model):
     def has_usable_password(self):
         """
         Return False if set_unusable_password() has been called for this user.
         """
         return is_password_usable(self.password)
 
     def get_session_auth_hash(self):
         """
         Return an HMAC of the password field.
         """
+        return self._get_session_auth_hash()
+
+    def get_session_auth_fallback_hash(self):
+        for fallback_secret in settings.SECRET_KEY_FALLBACKS:
+            yield self._get_session_auth_hash(secret=fallback_secret)
+
+    def _get_session_auth_hash(self, secret=None):
         key_salt = "django.contrib.auth.models.AbstractBaseUser.get_session_auth_hash"
         return salted_hmac(
             key_salt,
             self.password,
+            secret=secret,
             algorithm="sha256",
         ).hexdigest()
 
     @classmethod
     def get_email_field_name(cls):
         try:
             return cls.EMAIL_FIELD
         except AttributeError:
             return "email"
 
diff --git a/docs/ref/contrib/auth.txt b/docs/ref/contrib/auth.txt
index 241a0219bd..90ae5904a8 100644
--- a/docs/ref/contrib/auth.txt
+++ b/docs/ref/contrib/auth.txt
@@ -688,17 +688,24 @@ Utility functions
 .. function:: get_user(request)
 
     Returns the user model instance associated with the given ``request``’s
     session.
 
     It checks if the authentication backend stored in the session is present in
     :setting:`AUTHENTICATION_BACKENDS`. If so, it uses the backend's
     ``get_user()`` method to retrieve the user model instance and then verifies
     the session by calling the user model's
     :meth:`~django.contrib.auth.models.AbstractBaseUser.get_session_auth_hash`
-    method.
+    method. If the verification fails and :setting:`SECRET_KEY_FALLBACKS` are
+    provided, it verifies the session against each fallback key using
+    :meth:`~django.contrib.auth.models.AbstractBaseUser.\
+    get_session_auth_fallback_hash`.
 
     Returns an instance of :class:`~django.contrib.auth.models.AnonymousUser`
     if the authentication backend stored in the session is no longer in
     :setting:`AUTHENTICATION_BACKENDS`, if a user isn't returned by the
     backend's ``get_user()`` method, or if the session auth hash doesn't
     validate.
+
+    .. versionchanged:: 4.1.8
+
+        Fallback verification with :setting:`SECRET_KEY_FALLBACKS` was added.
diff --git a/docs/releases/4.1.8.txt b/docs/releases/4.1.8.txt
index 685580f33c..9f3dd167ed 100644
--- a/docs/releases/4.1.8.txt
+++ b/docs/releases/4.1.8.txt
@@ -2,11 +2,12 @@
 Django 4.1.8 release notes
 ==========================
 
 *Expected April 3, 2023*
 
 Django 4.1.8 fixes several bugs in 4.1.7.
 
 Bugfixes
 ========
 
-* ...
+* Fixed a bug in Django 4.1 that caused invalidation of sessions when rotating
+  secret keys with ``SECRET_KEY_FALLBACKS`` (:ticket:`34384`).
diff --git a/docs/topics/auth/customizing.txt b/docs/topics/auth/customizing.txt
index 3b688c8b5c..6cc48cacb1 100644
--- a/docs/topics/auth/customizing.txt
+++ b/docs/topics/auth/customizing.txt
@@ -715,20 +715,27 @@ The following attributes and methods are available on any subclass of
 
         Returns ``False`` if
         :meth:`~django.contrib.auth.models.AbstractBaseUser.set_unusable_password()` has
         been called for this user.
 
     .. method:: models.AbstractBaseUser.get_session_auth_hash()
 
         Returns an HMAC of the password field. Used for
         :ref:`session-invalidation-on-password-change`.
 
+    .. method:: models.AbstractBaseUser.get_session_auth_fallback_hash()
+
+        .. versionadded:: 4.1.8
+
+        Yields the HMAC of the password field using
+        :setting:`SECRET_KEY_FALLBACKS`. Used by ``get_user()``.
+
 :class:`~models.AbstractUser` subclasses :class:`~models.AbstractBaseUser`:
 
 .. class:: models.AbstractUser
 
     .. method:: clean()
 
         Normalizes the email by calling
         :meth:`.BaseUserManager.normalize_email`. If you override this method,
         be sure to call ``super()`` to retain the normalization.
 
diff --git a/tests/auth_tests/test_basic.py b/tests/auth_tests/test_basic.py
index 4b491e521e..c341aeb8c9 100644
--- a/tests/auth_tests/test_basic.py
+++ b/tests/auth_tests/test_basic.py
@@ -1,10 +1,11 @@
+from django.conf import settings
 from django.contrib.auth import get_user, get_user_model
 from django.contrib.auth.models import AnonymousUser, User
 from django.core.exceptions import ImproperlyConfigured
 from django.db import IntegrityError
 from django.http import HttpRequest
 from django.test import TestCase, override_settings
 from django.utils import translation
 
 from .models import CustomUser
 
@@ -131,10 +132,33 @@ class TestGetUser(TestCase):
     def test_get_user(self):
         created_user = User.objects.create_user(
             "testuser", "test@example.com", "testpw"
         )
         self.client.login(username="testuser", password="testpw")
         request = HttpRequest()
         request.session = self.client.session
         user = get_user(request)
         self.assertIsInstance(user, User)
         self.assertEqual(user.username, created_user.username)
+
+    def test_get_user_fallback_secret(self):
+        created_user = User.objects.create_user(
+            "testuser", "test@example.com", "testpw"
+        )
+        self.client.login(username="testuser", password="testpw")
+        request = HttpRequest()
+        request.session = self.client.session
+        prev_session_key = request.session.session_key
+        with override_settings(
+            SECRET_KEY="newsecret",
+            SECRET_KEY_FALLBACKS=[settings.SECRET_KEY],
+        ):
+            user = get_user(request)
+            self.assertIsInstance(user, User)
+            self.assertEqual(user.username, created_user.username)
+            self.assertNotEqual(request.session.session_key, prev_session_key)
+        # Remove the fallback secret.
+        # The session hash should be updated using the current secret.
+        with override_settings(SECRET_KEY="newsecret"):
+            user = get_user(request)
+            self.assertIsInstance(user, User)
+            self.assertEqual(user.username, created_user.username)

</diff>

<scope-context>
{
 "mode": "standalone",
 "base": "9b224579875e30203d079cc2fee83b116d98eb78",
 "diff_a": "9b224579875e30203d079cc2fee83b116d98eb78",
 "diff_b": null,
 "head_sha": "2396933ca99c6bfb53bda9e53968760316646e01",
 "branch": "review-head",
 "tree_is_reviewed_head": true,
 "checkout": "/home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-005/clone",
 "remote_refs": null,
 "note": "standalone base: review of the current checkout; the working tree is the reviewed head (2396933ca9), so read-only inspection of workspace files is valid. The checkout is read-only: no writes, no git writes; set PYTHONDONTWRITEBYTECODE=1 for any Python run against it."
}
</scope-context>

<protected-subject-policy>
Return status "confirmed", "rejected", or "unresolved". Never use lack of disproof as evidence of confirmation.

Set protected_subject to the best-fitting key below, or JSON null when none applies:
- memory-safety: allocation sizes, buffer lengths, index bounds, use-after-free, invalid memory access, or null dereferences.
- concurrency: locks, atomics, data races, ordering, or synchronization whose failure can affect observable behavior.
- data-loss: destructive writes, deletes, truncation, overwrite-in-place, or irreversible migrations and backfills.
- authorization-authentication: identity, permissions, ownership, session/token handling, or privilege boundaries.
- injection: attacker-influenced or untrusted data that can alter SQL, commands, templates, paths, or markup across a trust boundary, including stored input. Text assembly alone is not proof.
- public-contract: an evidenced compatibility concern involving an externally consumed response field, status code, error path, default, message, or published signature. An internal export or intentional contract change alone is not a defect.
- secrets-exposure: hardcoded credentials, API keys, tokens, or private keys in source or configuration; credentials, session tokens, or personal data written to logs, error messages, URLs, or responses; secrets committed to a repository or shipped in a built artifact.
- cryptography: weak or broken algorithms and modes, a fast general-purpose hash used for passwords, static or predictable keys, salts, or IVs, disabled certificate or signature verification, insufficient randomness, or a misused primitive whose failure breaks a security guarantee.

For every subject, confirm only when inspected evidence establishes the issue, the diff introduces or newly exposes it, and surrounding code or applicable runtime guarantees do not prevent it.

A finding that is real in the code may still describe a state that never occurs. For every finding, name the precondition the defect needs (the input, data shape, or ordering) and say what would show it occurs or is reachable: a test, a query against available data, a caller that produces it. When that evidence is in reach with the budget, obtain it; when it is not, confirm on the code alone and state in `reason` that incidence was not measured. Unmeasured incidence does not lower confidence or block confirmation; it is what the reader needs to weigh the severity.

Treat a finding as protected when the actual failure it alleges falls within a subject above. Read its category, title, and body together; keywords only prompt inspection and never establish protection. A naming preference about a token helper is not a token-handling defect. Your classification cannot remove protection established by the claim; the consumer applies this test independently.

On a protected subject, reject only by citing specific evidence that refutes the claim or establishes that it is unrelated pre-existing behavior: quote the file and line number that refutes it, name the version-specific or configuration-specific documentation and the version in force, give short-hash provenance, or cite a discriminating test result. Test evidence must identify the reviewed revision, engine/runtime version, configuration, exercised trigger, assertion, and observed result, and explain why it disproves the exact claim. A general passing suite or a test that did not exercise the alleged trigger is not disproof. An assumed framework guarantee is not evidence. Without one of these evidence forms, return status "unresolved", not "rejected". Inspect existing test evidence or use a read-only reproduction within your authority; do not mutate files or application state to obtain it.

If a protected claim remains uncertain, return status "unresolved" and state the missing evidence. Low confidence alone does not justify rejecting or confirming it.

Outside protected subjects, keep the ordinary conservative evidence bar: after inspection, reject an unsupported claim and explain why. Missing required inspection is different from inspected-but-unsupported evidence. If the cited file or required context cannot be accessed, return status "unresolved" for any subject, state the access limit, and do not guess.

Classify the claim itself, not its title. Never raise severity or confidence to preserve it. Do not invent findings or propose that uncertainty is a confirmed defect.
</protected-subject-policy>

For local-aligned scope, inspect the cited files, callers, guards, project contracts, and targeted history with read-only tools. For pr-remote or branch-remote scope, use the provided diff and reviewed head ref, never the unrelated workspace copy.

Budget: the batch has 15 minutes of wall clock and about five tool calls per finding. Inspect findings in the order given. When the budget runs out, stop inspecting and give every remaining finding `"status": "unresolved"` with the reason `budget exhausted, uninspected`; never guess a verdict you did not inspect.

Write the JSON object below to `/home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-005/clone-work/ce-review-artifacts/ce-code-review/20261028-180105-de59d602/validator-verdicts.json` before you return, then return the same object:
{
  "verdicts": [
    {
      "#": <input stable number>,
      "status": "confirmed" | "rejected" | "unresolved",
      "protected_subject": "<one of the eight policy keys>" | null,
      "reason": "<one sentence grounded in inspected evidence, or naming the evidence you could not obtain>"
    }
  ]
}

Each entry carries exactly those four fields. Return one verdict for every input # exactly once; unknown, duplicate, or missing numbers and invalid status or subject values are malformed output. Do not emit the legacy `validated` boolean. No prose outside JSON. Writing the verdicts file above is the one permitted write; do not edit project files, commit, push, or otherwise mutate the checkout.

Execution environment: checkout /home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-005/clone is strictly read-only (no file or git writes; PYTHONDONTWRITEBYTECODE=1 for any Python). Scratch files only under /home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-005/tmp/validator/. Venv: /home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-005/clone-cache/venv/bin/python. You may run standalone scripts in scratch with PYTHONPATH=<checkout>; do not run tests/runtests.py selections that reviewers already ran (auth_tests, sessions_tests); a single narrower selection such as auth_tests.test_basic is allowed once. No network. Launch no subagents, invoke no skills.
