You are the independent validation gate for the code-review findings below. Evaluate every finding separately under fresh inspection and under the shared protected-subject policy below. Outside a protected subject, false positives are common; reject a finding when the cited code does not prove it, it predates and is unaffected by this diff, surrounding code handles it, or it is only an unsupported preference. Inside a protected subject, a rejection requires one of the evidence forms the policy names.

Do not let one finding's outcome influence another. Do not invent new findings.

<findings-to-validate>
[
  {
    "#": 1,
    "severity": "P1",
    "category": "correctness",
    "title": "get_user crashes with AttributeError for duck-typed users on hash mismatch",
    "file": "django/contrib/auth/__init__.py",
    "line": 215,
    "confidence": 100,
    "why_it_matters": "Django documents that a user object only needs its own get_session_auth_hash() to take part in session verification; it does not have to inherit AbstractBaseUser. For such a user, a stale session hash used to cause a clean logout. Now it raises AttributeError (HTTP 500) because get_user() calls user.get_session_auth_fallback_hash(), and only AbstractBaseUser defines that method. A stale hash happens whenever the password is changed from another device, and also on every SECRET_KEY rotation. The session is never flushed because the exception fires before flush(), so every later request that touches request.user for that session fails with a 500 until the cookie expires or is cleared. This would ship in a 4.1.x/4.2 patch release. Guarding the fallback lookup the same way the hasattr check guards get_session_auth_hash keeps the old flush-and-logout behavior for these users.",
    "evidence": [
      "django/contrib/auth/__init__.py:215 -- for fallback_auth_hash in user.get_session_auth_fallback_hash()",
      "django/contrib/auth/__init__.py:200 -- if hasattr(user, \"get_session_auth_hash\"):",
      "docs/topics/auth/default.txt:920-923 -- If your AUTH_USER_MODEL inherits from AbstractBaseUser or implements its own get_session_auth_hash() method, authenticated sessions will include the hash returned by this function.",
      "Reproduction (scratch-api-contract/duck.py): user class defining only get_session_auth_hash, session hash 'stale' -> get_user raised AttributeError: 'U' object has no attribute 'get_session_auth_fallback_hash' (pre-change code path would flush and return AnonymousUser)",
      "django/contrib/auth/__init__.py:200 -- if hasattr(user, \"get_session_auth_hash\"):  (the only guard; it checks for get_session_auth_hash, not for the fallback method)",
      "docs/topics/auth/default.txt:920-923 -- \"If your AUTH_USER_MODEL inherits from AbstractBaseUser or implements its own get_session_auth_hash() method, authenticated sessions will include the hash returned by this function.\"",
      "django/contrib/auth/base_user.py:141 -- def get_session_auth_fallback_hash(self):  (defined only on AbstractBaseUser; grep finds no other definition)",
      "Scenario: a user object implementing get_session_auth_hash but not inheriting AbstractBaseUser logs in. Its password is changed elsewhere, so the stored HASH_SESSION_KEY no longer matches. On the next request, get_user reaches line 213 with session_hash truthy and evaluates user.get_session_auth_fallback_hash(), which raises AttributeError. flush() at line 219 never runs, so the session keeps its stale state and every later request repeats the crash.",
      "docs/topics/auth/default.txt:920-922 -- If your AUTH_USER_MODEL inherits from AbstractBaseUser or implements its own get_session_auth_hash() method, authenticated sessions will include the hash returned by this function.",
      "Scratch repro (scratch-security/duck.py): user object with only get_session_auth_hash and a mismatched stored hash -> AttributeError 'DuckUser' object has no attribute 'get_session_auth_fallback_hash'; session after call still contains _auth_user_id/_auth_user_hash (not flushed).",
      "django/contrib/auth/base_user.py:141 -- def get_session_auth_fallback_hash(self):  (defined only on AbstractBaseUser)",
      "Pre-diff behavior: mismatch -> request.session.flush(); user = None (no extra attribute access)."
    ],
    "first_evidence": "django/contrib/auth/__init__.py:215 -- for fallback_auth_hash in user.get_session_auth_fallback_hash()",
    "suggested_fix": "Guard the fallback lookup so the new method is optional, matching the existing hasattr pattern: compute `fallback_hashes = user.get_session_auth_fallback_hash() if hasattr(user, \"get_session_auth_fallback_hash\") else ()` and iterate over that in the any(...). Add a test in tests/auth_tests/test_basic.py using a user/backend whose user defines only get_session_auth_hash and a mismatched session hash, asserting AnonymousUser is returned and the session is flushed.",
    "autofix_class": "gated_auto",
    "owner": "downstream-resolver",
    "requires_verification": true,
    "reviewers": [
      "adversarial",
      "api-contract",
      "correctness",
      "security"
    ]
  },
  {
    "#": 2,
    "severity": "P2",
    "category": "testing",
    "title": "No test rejects non-matching hash when fallbacks are configured",
    "file": "django/contrib/auth/__init__.py",
    "line": 213,
    "confidence": 75,
    "why_it_matters": "No test checks the rule this change must keep: with SECRET_KEY_FALLBACKS set, a session whose hash matches neither the current key nor any fallback (for example after a password change) still gets flushed. The existing rejection tests (test_middleware.test_changed_password_invalidates_session, TestGetUser) run with the default empty SECRET_KEY_FALLBACKS, so they never reach the new reject path behind a non-empty fallback list. In a scratch copy of the reviewed tree I replaced the any(constant_time_compare(...)) check with `if session_hash and settings.SECRET_KEY_FALLBACKS:`, which accepts every session whenever fallbacks exist. All 984 auth_tests + sessions_tests still passed. So a regression that keeps password-changed sessions alive during key rotation, the exact security property the intent says must not weaken, would ship undetected.",
    "evidence": [
      "django/contrib/auth/__init__.py:213 -- if session_hash and any(",
      "django/contrib/auth/__init__.py:214-215 -- constant_time_compare(session_hash, fallback_auth_hash) for fallback_auth_hash in user.get_session_auth_fallback_hash()",
      "tests/auth_tests/test_middleware.py:27-35 -- test_changed_password_invalidates_session runs with default (empty) SECRET_KEY_FALLBACKS, so it never reaches the any() over fallbacks",
      "tests/auth_tests/test_basic.py:151-153 -- the only test with SECRET_KEY_FALLBACKS set is the positive-match case",
      "Mutation (scratch copy at HEAD 2396933ca9): replacing the any(...) condition with `settings.SECRET_KEY_FALLBACKS` -> `runtests.py auth_tests sessions_tests`: Ran 984 tests, OK"
    ],
    "first_evidence": "django/contrib/auth/__init__.py:213 -- if session_hash and any(",
    "suggested_fix": "Add a negative case to TestGetUser in tests/auth_tests/test_basic.py: log in, then change the password (user.set_password('new'); user.save()) and call get_user(request) under override_settings(SECRET_KEY='newsecret', SECRET_KEY_FALLBACKS=[settings.SECRET_KEY]). Assert the result is AnonymousUser and request.session.session_key is None. Also add a case where the session was signed with a key absent from both SECRET_KEY and SECRET_KEY_FALLBACKS (e.g. SECRET_KEY='newsecret', SECRET_KEY_FALLBACKS=['unrelated']) and assert the same flush.",
    "autofix_class": "gated_auto",
    "owner": "downstream-resolver",
    "requires_verification": true,
    "reviewers": [
      "testing"
    ]
  }
]
</findings-to-validate>

<diff>
Read the staged diff file: /home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-010/clone-work/ce-review-artifacts/ce-code-review/20261028-190240-9a426c5b/full.diff
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
  "repo_root": "/home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-010/clone",
  "diff": "/home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-010/clone-work/ce-review-artifacts/ce-code-review/20261028-190240-9a426c5b/full.diff",
  "files": "/home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-010/clone-work/ce-review-artifacts/ce-code-review/20261028-190240-9a426c5b/files.txt",
  "pr": {
    "number": 16631,
    "url": "https://github.com/django/django/pull/16631",
    "title": "Fixed #34384 -- Fixed session validation when rotation secret keys."
  },
  "note": "Standalone base: review of the current checkout; the working tree is the reviewed head, so read-only local inspection of cited files, callers, and history is valid. Read-only only: do not modify the checkout."
}
Scope mode is standalone (equivalent to local-aligned for inspection): Read/Grep the workspace at repo_root read-only. Optional read-only test runs: from repo_root, PYTHONPATH=<repo_root> /home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-010/clone-cache/venv/bin/python tests/runtests.py <selection> --settings=test_sqlite (5-minute limit, each selection at most once). Any scratch files only under /home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-010/clone-work/ce-review-artifacts/ce-code-review/20261028-190240-9a426c5b/scratch-validator/. Never modify the checkout.
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

Write the JSON object below to `/home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-010/clone-work/ce-review-artifacts/ce-code-review/20261028-190240-9a426c5b/validator-verdicts.json` before you return, then return the same object:
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