# TLS adapter: context ownership, effective policy, and initialization

## Scope and measurements

The only changed subsystem is `src/requests/adapters.py`. The review inspected the entire adapter, its Session call path, the default CA and ZIP extraction utilities, adapter extension documentation, existing TLS tests, and the locally installed urllib3 implementation. Repository guidance files and ambient configuration were not loaded as instructions.

The committed range is `8dd3b26bf59808de24fd654699f592abf6de581e..4089f3dc65f783beaa53cc032958ab625440d0ac`. The direct inspection commands were:

```sh
git diff main...review-head
git diff --numstat main...review-head
git show main:src/requests/adapters.py | wc -l
wc -l src/requests/adapters.py
nl -ba src/requests/adapters.py
```

The diff reports 28 additions and 18 deletions. The file grows from 606 to 616 lines. There is no 1,000-line crossing or reason to demand module decomposition based on size. The substantive growth is a shared context plus request-policy branching, rather than a giant-file problem.

The changed request helper now has separate branches for disabled verification, the cached default, custom CA files, and custom CA directories. The file/directory classification is also performed in `cert_verify()`. That duplication already existed for files; this PR adds directory handling to the helper while keeping a second classification and a comment-dependent default-context invariant in the later hook. The important simplification is to resolve effective policy once, not to extract every conditional into a new wrapper.

## Client identity ownership

**Finding anchor: src/requests/adapters.py:94–95.** The new default context is passed for every exact `verify=True`, regardless of `client_cert`. The same helper independently supplies `cert_file` and `key_file` at lines 102–109. A different client certificate can therefore select a different pool while still receiving the identical SSLContext object.

The complete local call path is:

1. `HTTPAdapter.send()` obtains a pool through `_get_connection()` at line 532.
2. `_get_connection()` builds request TLS kwargs at line 383 and passes them to the selected manager at lines 395–402.
3. HTTPS pools propagate the context and certificate paths to connections. In installed urllib3, `connectionpool.py:1099–1114` constructs the connection with those fields.
4. urllib3’s `connection.py:1031–1040` reuses a supplied context instead of constructing a new one.
5. `util/ssl_.py:429–433` calls `context.load_cert_chain(certfile, keyfile)` when a certificate file is configured.
6. When the next request has no certificate file, that branch does nothing. It does not remove an identity previously loaded into the context.

At the base, ordinary connections have no supplied context, so urllib3 constructs one for the connection; the first identity does not become the second Session’s identity. The head makes that identity process-wide for default verified requests.

The scratch probe `../review-probes/test_tls_ownership.py::test_client_identity_does_not_escape_into_unrelated_session` generates a CA, server certificate, and client identity with trustme. It substitutes the generated CA for the module’s default trust source, preserving the head’s shared-context ownership model. This avoids depending on a public CA’s private key or modifying the real certifi bundle.

A local TLS server requests an optional client certificate and records the peer certificate on two connections. Session A requests with `cert=<client PEM>`; Session B is newly constructed and requests with `cert=None`. Each Session mounts an adapter from the source version being tested, ignores environment proxies, and closes after its request. The server response also closes each TCP connection, so pooled connection reuse cannot explain the result.

The base sends an identity only on the first connection. The head sends an identity on the second connection too, failing the assertion that its peer certificate should be absent. Both HTTP exchanges succeed, so this failure directly measures authentication state escaping its intended owner.

Concurrent identity overwrite follows from sharing the mutable destination of `load_cert_chain()`; concurrency itself was not exercised. The sequential reproduction is sufficient to reject the design without relying on a scheduling race.

The existing repository mTLS test does not cover this invariant. It first uses `verify=False` and then expects `verify=True` to fail server validation. It does not make two successful default verified connections with distinct client policies, so a green version of that test would not establish isolation.

### Worked code-judo proposal

Keep one reusable context for the ordinary default policy, and let urllib3 retain its normal per-connection context ownership whenever a request or adapter supplies client identity. This preserves the common-case performance objective and deletes the need for a global context cache keyed by every identity.

The essential request-policy change is:

```python
# Policy sketch, not a patch: adapter defaults also need the check below.
if verify is True:
    if client_cert is None and not adapter_has_tls_overrides:
        pool_kwargs["ssl_context"] = get_preloaded_default_context()
    else:
        pool_kwargs["ca_certs"] = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
```

The explicit CA path on the fallback is necessary. Merely excluding client certificates from the shared context would let urllib3 use OS defaults where Requests previously specified certifi. Retaining the default trust source is part of preserving behavior.

Also account for client identity supplied through manager defaults, such as `cert_file` or `key_file`, rather than just the request’s `cert` argument. Do not attempt to clear the shared context after each request or put a lock around only certificate loading: an identity remains installed after the lock is released, and other handshakes still share it.

Verification needed for a remedy includes A→none, A→B, separate Sessions, separate origins, and concurrent identities. The existing probe covers A→none across Sessions. No proposed remedy was implemented.

## Adapter configuration boundary

**Finding anchor: src/requests/adapters.py:94–95.** `init_poolmanager()` is explicitly exposed for adapter subclasses at lines 221–234. Its extra kwargs become `PoolManager.connection_pool_kw`. The new request helper cannot inspect those defaults because it only receives the request, verification setting, and client certificate.

Installed urllib3’s `poolmanager.py:390–410` copies manager defaults and then overwrites them with request kwargs. Consequently, the newly supplied `ssl_context` is an override, not a fallback.

The probe `test_adapter_owned_context_survives_default_verification` defines an adapter subclass whose `init_poolmanager()` passes a specific context to its superclass. It prepares an HTTPS request, obtains the pool through `_get_connection(verify=True)`, and checks the pool’s context by identity. At the base, the pool contains the subclass’s context. At the head, the pool contains the new module context instead.

This probe requires no socket or trust fixture. It verifies context selection directly; it does not claim to have executed every custom cipher, trust store, or TLS version setting. Those consequences follow from replacing the object that owns them.

There is a second manifestation of the same boundary problem: `connection.py:1031–1038` applies `ssl_version`, `ssl_minimum_version`, and `ssl_maximum_version` when it constructs a context, but lines 1039–1040 bypass that construction if a context was supplied. Existing adapter settings can remain present in the pool while ceasing to configure the context. The repository’s `docs/user/advanced.rst:1012–1033` documents this adapter-level customization pattern using `ssl_version`. The historical SSLv3 example is architectural evidence; SSLv3 was not exercised here.

### Worked code-judo proposal

Use the existing helper as the canonical policy-resolution boundary, and give it the effective selected manager’s TLS defaults. Select the direct or proxy manager before resolving whether the ordinary default optimization applies. Resolve one result that is used to select the pool and configure its connections.

Cache eligibility should be an explicit conservative decision. The shared default is eligible only for default verified HTTPS without adapter-specific context, protocol, trust, identity, or hostname/fingerprint settings that can alter context behavior. For the installed dependency, relevant keys include `ssl_context`, `ssl_version`, `ssl_minimum_version`, `ssl_maximum_version`, `ca_certs`, `ca_cert_dir`, `ca_cert_data`, `cert_file`, `key_file`, `key_password`, `assert_hostname`, and `assert_fingerprint`. Confirm the appropriate set across supported urllib3 versions before implementing it.

For a supplied context, keep that object and the legacy default-bundle behavior instead of inserting the cached context over it. For protocol settings without a supplied context, keep urllib3’s construction path and specify the proper CA source. This avoids trying to mutate the module context into each adapter’s policy.

Consolidation must preserve the documented `cert_verify()` subclass hook. Its existing direct-use and override contract makes deleting it wholesale unsafe. A compatibility facade can retain historical validation and configuration for callers outside the new normalized flow, while the ordinary send path uses one effective policy. The intended reduction is one owner for CA selection and cache eligibility, not a second parallel policy model.

Test proxy-manager customization independently because that manager can carry its own defaults. Validate adapter-supplied context identity and an actual nondefault protocol constraint. These remedy tests were not executed.

## Import and resource lifetime

**Finding anchor: src/requests/adapters.py:75–78.** The new initializer constructs a context and loads the default bundle while the module is being imported. `requests.__init__` imports API and Session modules, and `sessions.py` imports HTTPAdapter, so an initializer error can prevent `import requests` from completing.

At the base, `cert_verify()` resolves and validates the default bundle only in the HTTPS verification branch. Plain HTTP, disabled verification, and an explicitly supplied CA path can avoid opening that default bundle. `Session.merge_environment_settings()` at lines 765–780 also resolves `REQUESTS_CA_BUNDLE` and `CURL_CA_BUNDLE` at request time, too late to rescue the new import-time initializer.

The missing-bundle probe patches `requests.utils.DEFAULT_CA_BUNDLE_PATH` to a nonexistent path and imports a fresh module from each adapter source. The base module imports successfully. The head raises `FileNotFoundError` at line 76. This directly tests the initializer boundary; it is not a fresh-process top-level Requests import. The package import consequence is established by the source import chain.

ZIP extraction itself was retained through the canonical `extract_zipped_paths()` utility; no ZIP-support regression is asserted. The problem is that extraction and bundle parsing are now prerequisites for all adapter imports, even when the caller’s selected policy will never use this trust store.

### Worked code-judo proposal

Separate the lifetime of reusable default TLS state from module import. Make one private default-context accessor, called only after the resolver selects ordinary default verified HTTPS. Synchronize first use so concurrent first requests do not duplicate expensive loading, and assign the cached object only after full successful setup.

```python
# Initialization sketch, not an applied or tested implementation.
_default_context = None
_default_context_lock = Lock()

def get_preloaded_default_context():
    global _default_context
    with _default_context_lock:
        if _default_context is None:
            bundle = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
            if not bundle or not os.path.exists(bundle):
                raise OSError(
                    "Could not find a suitable TLS CA certificate bundle, "
                    f"invalid path: {bundle}"
                )
            context = create_urllib3_context()
            context.load_verify_locations(bundle)
            _default_context = context
        return _default_context
```

The lock here protects initialization and publication, not mutations made by identity-bearing connections. The first finding’s eligibility restriction is still required. A directory-valued default bundle, if supported by the existing default-resolution contract, needs the corresponding `capath` handling rather than a blind `cafile` call.

This accessor earns its indirection by owning lazy resource acquisition, synchronization, and atomic publication. No general policy class or arbitrary context registry is necessary for this small optimization.

Preserve a useful default-bundle diagnostic when default verified HTTPS actually needs the missing resource. Allow import and alternate policies to proceed without entering this accessor. A loader failure leaves the cache empty, allowing a subsequent valid attempt rather than publishing incomplete state.

Remedy verification should count loads under simultaneous first use and repeated ordinary connections, exercise the retained ZIP utility, and test a missing default bundle with HTTP, disabled verification, and explicit/environment CA bundles. Those checks are proposed, not claimed as completed.

## Verification record

All commands ran from the clone root within the five-minute execution allowance. No test accessed a public host. Python bytecode writes and pytest’s cache provider were disabled. All new probes, extracted base source, and temporary test data reside in the work directory.

The base adapter was obtained without changing the checkout:

```sh
git show main:src/requests/adapters.py > ../clone-work/review-probes/base_adapters.py
```

The environment prefix used for every pytest execution was:

```sh
PYTHONDONTWRITEBYTECODE=1
PYTHONPATH=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-001/clone/src
```

These variables were supplied inline to the pre-provisioned Python command, not merely assigned in an unexported shell. The executable was `../clone-cache/venv/bin/python`. The concrete selections and flags were:

```sh
../clone-cache/venv/bin/python -m pytest \
  ../clone-work/review-probes/test_tls_ownership.py \
  -q -p no:cacheprovider -o addopts= \
  --basetemp=../clone-work/review-probes/probe-temp --tb=short

../clone-cache/venv/bin/python -m pytest tests/test_requests.py \
  -k 'ssl or verify or cert_bundle or certificate or mtls' \
  -q -p no:cacheprovider -o addopts= \
  --basetemp=../clone-work/review-probes/existing-temp --tb=short

../clone-cache/venv/bin/python -m pytest \
  ../clone-work/review-probes/test_existing_mtls_base.py \
  -q -s -p no:cacheprovider -o addopts= \
  --basetemp=../clone-work/review-probes/base-fixture-temp \
  --tb=short --disable-warnings
```

The ownership selection completed in 0.19 seconds: 3 passed base cases and 3 failed head cases. The failures were context identity replacement, `FileNotFoundError` during adapter import, and an unexpected peer certificate on the second Session’s connection.

The repository TLS selection completed in 2.40 seconds: 15 passed, 3 failed, and 311 deselected. `test_pyopenssl_redirect` and `test_different_connection_pool_for_tls_settings_verify_bundle_unexpired_cert` failed with the packet’s known “Missing Authority Key Identifier” problem.

The third repository failure was `test_different_connection_pool_for_mtls_settings`, whose initial `verify=False` request failed with a certificate-expired alert. The additional base-fixture probe replaced Session’s adapter constructor with the extracted base implementation and invoked that original test method unchanged. It printed the client fixture’s `notAfter` as `Mar 13 18:35:04 2026 GMT`, then reproduced the same error through the base adapter. It completed in 0.33 seconds with one failure. This baseline reproduction was prompted by a newly observed failure, rather than a repeated unchanged selection.

The generated-cert identity test avoids these historical fixture limitations. The adapter-context probe inspects pool state and the import probe inspects initializer behavior; neither needs network access.

Installed dependencies were read locally. The venv is Python 3.13, and urllib3’s package metadata reports 2.8.0. These source traces and executions substantiate behavior in this environment. The supported dependency range in `setup.py` is `urllib3>=1.21.1,<3`; its other versions were not installed or tested. No performance benchmark, full-suite run, or candidate-remedy execution was performed.

## Checkout integrity and review boundaries

Initial status was clean and HEAD was the pinned review head. After test execution, `git status --porcelain=v1 --untracked-files=all` produced no entries, and both `git diff --exit-code` and `git diff --cached --exit-code` were clean. HEAD remained `4089f3dc65f783beaa53cc032958ab625440d0ac`, with tree `d1febc2d2f8b1b2877afe593728d74c6f71a81f3`.

No checkout remedy was applied. Proposed code is design material only. There are three actionable findings, all summarized in `summary.md`; no separate question or cosmetic finding is being introduced here.

