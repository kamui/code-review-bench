# TLS context ownership and lifecycle

## Scope and structural assessment

The patch changes only `src/requests/adapters.py`. Its structure is compact, but it crosses two ownership boundaries: the existing per-adapter urllib3 manager configuration and the module's import lifecycle. `_urllib3_request_context()` currently chooses a single module-global context for all ordinary verified requests, while the module itself loads the default trust bundle as soon as it is imported.

There is a code-judo path: make the adapter or manager the owner of its effective context, set a shared default only when the caller did not configure one, and initialize that default only when verified TLS is actually requested. This keeps context reuse without making every request overwrite customization or every import do TLS setup.

## Finding 1: Per-request context overrides adapter customization

**Evidence.** In `src/requests/adapters.py:75-78`, `_preloaded_ssl_context` is created globally. In `src/requests/adapters.py:94-95`, `_urllib3_request_context()` puts it in `pool_kwargs` whenever `verify is True`. `HTTPAdapter.init_poolmanager()` at `src/requests/adapters.py:218-247` forwards arbitrary `pool_kwargs` into `PoolManager`, an established subclass hook. The request then calls `connection_from_host(..., pool_kwargs=pool_kwargs)` at `src/requests/adapters.py:383-395`. The installed urllib3 implementation merges these request kwargs over `connection_pool_kw` (`poolmanager.py`, `_merge_pool_kwargs`), so this request-level `ssl_context` wins over an adapter's configured context. The same pattern is used when obtaining a proxy manager connection.

**Why this matters.** An adapter subclass can provide an SSL context with custom trust roots, TLS versions, ciphers, or other settings through `init_poolmanager(..., ssl_context=...)`. With this patch, `verify=True` silently sends ordinary verified requests through the module's default context instead. That breaks the purpose of the customization hook and makes the manager's configured state misleading. The issue is a boundary problem: per-request default selection leaks across the adapter's explicit configuration.

**Worked code-judo proposal.** Choose the context after `_get_connection()` has selected the direct or proxy manager, but before it asks that manager for the pool. Add the shared default to request kwargs only for a verified request whose selected manager has no configured context:

```python
def _get_connection(self, request, verify, proxies=None, cert=None):
    manager = self._manager_for(request.url, proxies)
    host_params, pool_kwargs = _urllib3_request_context(request, verify, cert)
    if verify is True and "ssl_context" not in manager.connection_pool_kw:
        pool_kwargs["ssl_context"] = _get_default_context()
    return manager.connection_from_host(**host_params, pool_kwargs=pool_kwargs)
```

The sketch elides the existing proxy URL validation and error translation; retain those around manager selection. Apply the same rule to SOCKS and HTTP proxy managers. Then remove the unconditional per-request context assignment from `_urllib3_request_context()`. Custom CA bundle paths remain request-specific and continue to create distinct pools; an explicitly configured manager context remains authoritative. If explicit `None` needs different semantics from an absent key, define that contract at the adapter boundary rather than silently overriding it downstream. This puts context selection at the only point that knows both the request's verification mode and the actual manager, and it allows the lazy helper in Finding 2 to stay unused until a verified request arrives.

**Verification status.** Inspection of the local urllib3 source confirms override precedence: `_merge_pool_kwargs()` copies manager kwargs and applies request override values afterward. The focused tests exercised pool separation for `verify=True` and a custom expired bundle successfully, but neither test covers a subclass-supplied SSL context. Add a regression test with a deliberately distinct context and assert it reaches the created HTTPS connection pool for direct and proxied requests.

## Finding 2: Certificate loading becomes unconditional import work

**Evidence.** `src/requests/adapters.py:75-78` creates the SSL context and immediately calls `load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))` at module scope. `requests` imports the adapter module as part of its normal package import. This means the CA bundle is resolved, potentially extracted from a zip, and loaded for every process importing Requests regardless of whether it ever makes a verified HTTPS request.

**Why this matters.** The PR addresses repeated `load_verify_locations()` calls during concurrent verified requests, but this implementation also makes every consumer pay the operation during import. HTTP-only tools and applications that always use `verify=False` get no benefit from a verified cached context but inherit its initialization cost and any certifi/zip-path setup failure at import time. The design replaces a request-path cost with an unconditional process-startup cost rather than limiting work to consumers that need it.

**Worked code-judo proposal.** Keep one cached context, but make its first construction demand-driven. A single helper provides the boundary and keeps the initialization logic in one place:

```python
_default_context = None
_default_context_lock = threading.Lock()

def _get_default_context():
    global _default_context
    if _default_context is None:
        with _default_context_lock:
            if _default_context is None:
                context = create_urllib3_context()
                context.load_verify_locations(
                    extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
                )
                _default_context = context
    return _default_context
```

Call this only when configuring a verified HTTPS manager that lacks an explicit context. This ensures contexts are fully initialized before sharing, avoids exposing a partially configured object to concurrent callers, and leaves non-TLS and unverified workloads free of CA-loading work. If an adapter has a shorter lifetime or custom trust roots, an adapter-owned cache can be used; the important simplification is to centralize the invariant instead of doing import-time setup plus per-request overrides.

**Verification status.** This is a direct control-flow finding, not a benchmark result: the module-level call is unconditional. No import-time benchmark was run. A regression test can reload/import the adapter while spying on context creation, then assert no context is loaded until verified HTTPS context setup is requested; a second call should reuse the cached context.

## Commands and observed results

The committed range was inspected with `git diff main...review-head -- src/requests/adapters.py`; the changed file is 616 lines, so this patch does not cross the skill's 1,000-line threshold. Relevant urllib3 pool merging code was read from the pre-provisioned environment.

Focused local execution:

```text
PYTHONPATH=<clone>/src <cache>/venv/bin/python -m pytest tests/test_requests.py -k 'different_connection_pool_for_tls_settings_verify_True or different_connection_pool_for_tls_settings_verify_bundle_expired_cert or different_connection_pool_for_mtls_settings'
```

Result: 2 passed, 1 failed, 326 deselected. The mTLS test's local server certificate is expired; handshake ended with `SSLV3_ALERT_CERTIFICATE_EXPIRED`. The other two selected TLS pool tests passed. This run does not verify custom `SSLContext` behavior or import cost.
