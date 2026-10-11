# TLS context and pool configuration

## Scope and measurements

The committed change is confined to `src/requests/adapters.py` (+28/−18). The file has 616 lines after the change, so it does not cross the skill's 1,000-line boundary. The main additions are the module-level `_preloaded_ssl_context` at lines 75–78 and the corresponding `verify` branches in `_urllib3_request_context()` at lines 91–101. `HTTPAdapter.cert_verify()` at lines 297–321 avoids assigning a default CA path for `verify=True`, while it still validates and assigns explicit CA paths.

The implementation keeps the transport-specific configuration in the adapter, where Requests already constructs urllib3 pools and applies certificate settings. The shared context is created once, loads the same default bundle that the prior request-time path extracted, and is only passed for the `verify is True` case. Explicit paths continue to select either `ca_certs` or `ca_cert_dir`, and `verify=False` retains `CERT_NONE`.

## Structural review and code-judo analysis

The apparent duplication between `_urllib3_request_context()` and `cert_verify()` is the only plausible cleanup target: both distinguish a CA file from a CA directory. However, the first function includes that choice in the pool kwargs, which lets urllib3 keep differently configured pools separate; the second preserves `cert_verify()`'s existing connection-configuration and path-validation behavior, including its documented subclassing role. Removing one branch would require changing that boundary or adding a new normalized representation, which introduces an abstraction without a demonstrated reduction in concepts. I found no stronger behavior-preserving restructuring that would materially simplify this focused change.

The module-level context is mutable in principle, but this code initializes its trust store once and does not reconfigure it after requests begin. The review packet records the project's discussion of sharing `SSLContext` across threads and the relevant caveat about changing context settings after use. No new branching is scattered into unrelated request flows, and no new module or helper would clearly improve locality.

## Findings

No actionable maintainability finding. In particular, the duplicated file-versus-directory classification is not reported as a finding because it supports two existing boundaries and no simpler equivalent was evident from the changed code.

## Verification status

Static inspection used:

```text
git diff --stat main...review-head
git diff --find-renames --unified=100 main...review-head -- src/requests/adapters.py
rg -n 'urllib3|python_requires|install_requires' setup.py setup.cfg pyproject.toml
rg -n 'verify|cert_verify|ca_cert|SSLContext|ssl_context|certifi|zip' tests/test_adapters.py tests/test_requests.py tests/test_packages.py
```

The focused test command was:

```text
PYTHONPATH=<clone>/src <clone-cache>/venv/bin/python -m pytest tests/test_requests.py -k 'tls_settings or invalid_ca_certificate_path or certificate_failure'
```

Result: 4 passed, 2 failed. `test_different_connection_pool_for_tls_settings_verify_bundle_unexpired_cert` is one of the fixture-certificate failures identified in the run packet as failing on both the merge-base and head under the installed OpenSSL. `test_different_connection_pool_for_mtls_settings` also failed in this run; the run packet does not identify it as a known baseline failure, and no baseline execution was performed, so this review does not attribute it to the patch. The other four selected tests passed. No performance benchmark was run.
