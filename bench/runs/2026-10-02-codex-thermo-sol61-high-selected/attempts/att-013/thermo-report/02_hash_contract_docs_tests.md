# Hash abstraction, documentation, and regression coverage

## Scope and verdict

This detail covers `django/contrib/auth/base_user.py`, all three changed documentation files, and `tests/auth_tests/test_basic.py`. The supporting implementation inspected was `django/utils/crypto.py`, `django/core/signing.py`, `django/contrib/auth/tokens.py`, and the auth model tests. No additional actionable finding was established in this subsystem. The missing compatibility test belongs to the finding in [01_session_validation.md](01_session_validation.md).

## Measurements

The pinned manifest contains six files, 69 insertions, and seven deletions. Line counts were measured against both revisions, rather than inferring size from changed-line counts alone.

| File | Base lines | Head lines | Threshold assessment |
| --- | ---: | ---: | --- |
| `django/contrib/auth/__init__.py` | 230 | 244 | Below 1,000 |
| `django/contrib/auth/base_user.py` | 158 | 167 | Below 1,000 |
| `docs/ref/contrib/auth.txt` | 704 | 711 | Below 1,000 |
| `docs/releases/4.1.8.txt` | 12 | 13 | Below 1,000 |
| `docs/topics/auth/customizing.txt` | 1,199 | 1,206 | Already above 1,000 |
| `tests/auth_tests/test_basic.py` | 140 | 164 | Below 1,000 |

Commands producing the measurements were:

```sh
git diff main...review-head --stat
for file in django/contrib/auth/__init__.py django/contrib/auth/base_user.py docs/ref/contrib/auth.txt docs/releases/4.1.8.txt docs/topics/auth/customizing.txt tests/auth_tests/test_basic.py; do
    git show main:"$file" | wc -l
    wc -l "$file"
done
```

No changed file crosses from below 1,000 to above 1,000 lines. The long customization document gains a seven-line method entry beside the existing session hash method. Splitting that documentation is not required to preserve cohesion for this change, and its existing size is not a new code-quality regression.

## Hash ownership and abstraction assessment

At `django/contrib/auth/base_user.py:135–152`, the existing public `get_session_auth_hash()` delegates to a secret-aware private helper. The fallback generator uses the same helper for each configured old key. The helper retains the preexisting salt string, password field, and SHA-256 algorithm; its only new input is the optional secret passed to the canonical `salted_hmac()` utility.

The public wrapper earns its place by keeping the zero-argument method stable. The private helper earns its place by putting the hash recipe in one location for current and historical keys. `secret=None` is the established crypto utility's default-key contract, not incidental optionality added to paper over an unclear data model. The generator is lazy, so the current-key success path computes no fallback hashes and a successful fallback comparison stops before later keys.

The ownership boundary is sensible: the user supplies hashes derived from its state, and auth orchestrates checking and session migration. Moving password-derived HMAC logic into `get_user()` would leak model-specific hashing into authentication orchestration. Replacing this with `Signer.unsign()` would also be incorrect without a format migration: session auth hashes are raw password HMACs, while signing uses a signed-value format and a different derivation/salt convention. The canonical reusable primitive here is `salted_hmac()`, which the PR already uses.

Password-reset token rotation uses an analogous secret-aware internal constructor, but its token format, timestamp, and user-state policy differ. Consolidating these into a generic multi-key verification framework would add abstraction without removing their genuinely different semantics. No helper duplication finding is justified.

## Customization boundary considered

Existing subclasses can override `get_session_auth_hash()` to change their session-validation recipe. The new inherited fallback method calls `_get_session_auth_hash()` rather than that public override. Therefore, an existing public override does not automatically gain equivalent key-rotation behavior. This is a customization limitation worth understanding when interpreting the feature; the private helper is the new shared recipe hook for implementations that use it.

The packet contains no concrete custom override affected by that limitation, and pre-PR rotation also invalidated those sessions. It is not reported as a second independently confirmed regression. This review does not recommend a secret-setting override, a global setting mutation, or reflection on arbitrary custom-method signatures to retrofit rotation. Those techniques would be more brittle than keeping the fallback capability explicit. The confirmed issue is narrower: a user without the new method must still invalidate a bad session safely.

## Documentation assessment

`docs/ref/contrib/auth.txt:698–711` describes fallback verification and marks the behavior as changed in 4.1.8. Its statement that an invalid hash returns `AnonymousUser` strengthens the importance of preserving the legacy invalidation boundary. The explicit method cross-reference points to the new model method documented by `docs/topics/auth/customizing.txt:725–730`. The reference's continuation syntax was inspected in source; no documentation build was run.

The customization entry accurately describes yielding password HMACs under fallback keys for the provided base implementation. `docs/releases/4.1.8.txt` replaces the placeholder bugfix with the session-invalidation fix and issue reference. The changes are focused and do not expose unrelated implementation machinery to readers. No separate actionable prose or release-note issue was identified.

## Tests and their limits

`tests/auth_tests/test_basic.py:143–164` adds a real user, logs in under the old secret, retrieves the user with a new key and the old fallback, asserts that the session key changes, and then removes the fallback while retaining the session object. This checks the central success case and demonstrates that the cached session hash is updated to the new key. Its import of `settings` captures the prior configured key before entering `override_settings()`; there is no global-settings mutation bug in the test.

The test does not instantiate a fresh session store after migration, cover a custom hash-only user, exercise multiple fallbacks, or test invalid hashes. Those coverage limits do not each become separate findings. Supplemental probes supplied the relevant negative and persistence checks, and the concrete missing custom-user case is included in the actionable finding's remedy.

The permitted existing-suite command was:

```sh
PYTHONDONTWRITEBYTECODE=1 \
PYTHONPATH=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-013/clone \
timeout 300 /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-013/clone-cache/venv/bin/python \
tests/runtests.py auth_tests.test_basic auth_tests.test_models --settings=test_sqlite --parallel=1
```

It passed all 64 tests in 0.806 seconds, with SQLite test databases for the default and other aliases and no system-check errors. This establishes that the selected existing model/auth tests accept the submitted implementation; it does not cover every authentication backend, session engine, or custom model. The scratch probes and candidate verification are documented in the session-validation detail. The full suite, documentation build, and alternate session-engine matrix were not executed.

## Preservation and execution policy

The review used offline repository reads and the pre-provisioned virtualenv. Scratch files and all reports were written outside the clone, under the attempt work directory. The frozen skill contained no requirement to invoke another skill or child reviewer; no delegation occurred.

Repository preservation was checked with:

```sh
git status --porcelain=v1
git diff --exit-code
git diff --cached --exit-code
git rev-parse HEAD HEAD^{tree}
git ls-files -s | sha256sum
```

The working tree and index were clean at the checks. The head SHA was `2396933ca99c6bfb53bda9e53968760316646e01`; the tree identity was `4e28cc9bf22bed440c3d42b0c1f6cb615aec849e`. The index-listing SHA-256 before and after test execution was `aa09c25de312102d3ac376f7fb2cc762a15b2cc6863c2b7d0c189e9f838b1b2b`. No remedies were applied.
