# TLS context and CA path policy

## Scope and evidence

The change adds `_preloaded_ssl_context` at module import and loads `extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)` into it (`src/requests/adapters.py:75–78`). `_urllib3_request_context()` then supplies that context for `verify is True` and derives `ca_certs` versus `ca_cert_dir` for string paths (`src/requests/adapters.py:91–101`). Later, `HTTPAdapter.cert_verify()` validates custom paths and independently repeats the file-versus-directory split (`src/requests/adapters.py:297–317`). The latter remains necessary for its adapter-facing behavior, but the new pool-key branch creates a second implementation of the same classification.

The file is 616 lines at head, below the skill's 1,000-line threshold. The diff adds no unrelated conditionals or extra module layers. The principal code-quality opportunity is to express custom CA location once, since pool-key configuration and connection configuration must describe the same trust source.

## Finding: duplicated CA path classification

`_urllib3_request_context()` classifies a string path with `os.path.isdir()` and writes either `ca_certs` or `ca_cert_dir`. `cert_verify()` repeats that check after confirming the path exists and writes the corresponding connection attribute. In the normal send flow both run for a custom CA path. That duplication introduces a synchronization obligation into a security-sensitive policy: a future change to accepted path types, symlink handling, normalization, or error behavior can update one branch and leave the other branch stale. If the filesystem classification changes between the two calls, the pool's key/configuration and the connection's attributes can also disagree.

**Action:** move the path-shape decision to one helper, such as `_ca_bundle_pool_kwargs(verify)`, returning either `{"ca_certs": path}` or `{"ca_cert_dir": path}` for a string path. Use that helper from `_urllib3_request_context()` and derive the connection attribute from the same result in `cert_verify()`. Keep existence validation in `cert_verify()` if preserving its current error timing is important. The helper should not load certificates; the SSL context and urllib3 remain the owners of trust loading. This deletes the second conditional without adding a policy object or broad abstraction.

A more complete simplification would let `_urllib3_request_context()` return a small explicit TLS settings value and have both pool selection and adapter validation consume it. That is only justified if this code gains more TLS modes; for the present two-way file/directory decision, a tiny shared helper is the direct option.

## Verification status

The command run was:

```text
PYTHONPATH=<clone>/src <cache>/venv/bin/python -m pytest tests/test_requests.py -k 'invalid_ca_certificate_path or certificate_failure or different_connection_pool_for_tls_settings_verify_True or different_connection_pool_for_tls_settings_verify_bundle_expired_cert or different_connection_pool_for_tls_settings_verify_bundle_unexpired_cert' -q
```

Result: 4 passed, 1 failed, 324 deselected. The failure was `test_different_connection_pool_for_tls_settings_verify_bundle_unexpired_cert`, with OpenSSL reporting `Missing Authority Key Identifier`. The packet states this fixture mismatch fails on the merge-base as well as the head, so it is not evidence against this change. `git diff --check main...review-head` completed cleanly. No benchmark was run and no benchmark claim is verified here.
