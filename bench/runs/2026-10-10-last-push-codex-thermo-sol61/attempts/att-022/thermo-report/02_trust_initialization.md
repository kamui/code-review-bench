# Default trust-store initialization and error boundaries

## Finding: eager loading makes a TLS resource an import prerequisite

The new `src/requests/adapters.py:75–78` constructs and loads a TLS context while the module is imported. `requests.__init__` imports its API, which imports sessions, which imports the adapter. An error from default-bundle loading therefore prevents importing Requests at all.

Before the patch, `cert_verify()` resolves and checks the default CA path only for verified HTTPS requests. HTTP requests, requests with `verify=False`, and requests with an explicit custom CA location do not need a readable default bundle. The patch moves that work ahead of every one of those choices. Missing, unreadable, or malformed default CA material can now fail unrelated importers before they can set Session verification, use an environment override, or install a custom adapter.

This is an abstraction-boundary regression: a reusable transport policy becomes a prerequisite for using a general HTTP library. Caching root certificates is appropriate; moving a potentially failing filesystem/OpenSSL operation into an unconditional module initializer is not required by that optimization. The default resource should be initialized when the effective TLS policy actually selects it.

## Source evidence

The removed code in base `cert_verify()` initializes `cert_loc`, resolves the default path through `extract_zipped_paths()` only when needed, validates existence, then selects `ca_certs` or `ca_cert_dir`. At the head, default resolution and OpenSSL parsing happen at lines 75–78, without a try/except or lazy boundary. The modified verification hook skips the default-path flow when `verify is True`.

`src/requests/utils.py::extract_zipped_paths()` returns an already valid filesystem path unchanged, extracts a valid archive member when appropriate, and returns the original path when it is neither an existing file nor a valid archive member. It does not guarantee the result is usable CA material. Keeping that canonical helper in the new code correctly preserves extraction; it does not make the subsequent import-time loading infallible.

The file-versus-directory assumption is not separately flagged for the default certifi path: the packet identifies the default as a certifi bundle file, and the changed code intentionally preserves extraction. Nor is the mere existence of a process-wide root cache a finding. The actionable problem is when and under which policy that cache is allowed to fail.

## Offline reproduction and verification status

`../review-checks/test_context_boundaries.py::test_missing_default_bundle_does_not_prevent_import` starts a fresh interpreter, imports certifi, and replaces `certifi.where()` with a scratch nonexistent filename before importing Requests. This models an unavailable default bundle without deleting or modifying any installed package or checkout file.

For the base case, a scratch import finder supplies the exact committed base `requests.adapters` source. All other Requests modules and installed dependencies are the same as the head case; adapters.py is the only changed file in this PR. The subprocess imports Requests, creates a Session, and sets `session.verify = False`.

Base: PASS, with import and Session construction succeeding. Head: FAIL, with `FileNotFoundError` originating at `adapters.py:76` during import, before a Session can be constructed. This directly verifies the changed error boundary. Actual unreadable and malformed CA files were not separately exercised, and an HTTP socket request was unnecessary because the import itself already fails.

The command was the initial six-case boundary selection documented in [01_tls_ownership.md](01_tls_ownership.md). Its output is retained in `../review-checks/boundary-results.txt`; both import cases are valid even though a separate identity test in that initial run needed a harness correction. No subprocess fetched data or contacted a live host.

## Worked code-judo proposal

Move the global from a fully initialized resource to a lazily published default-policy cache. Resolve whether a request qualifies for that policy before calling the factory. This avoids adding exception handlers that silently substitute a different trust store or disable verification when import-time loading fails.

A direct internal initialization sketch is:

```python
# Conceptual flow; not an applied or validated patch.
_default_context = None
_default_context_lock = threading.Lock()

def get_default_ssl_context():
    global _default_context
    with _default_context_lock:
        if _default_context is None:
            path = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
            if not path or not os.path.exists(path):
                raise OSError(
                    "Could not find a suitable TLS CA certificate bundle, "
                    f"invalid path: {path}"
                )
            context = create_urllib3_context()
            context.load_verify_locations(path)
            _default_context = context
        return _default_context
```

Construct in a local variable and publish only after successful loading. A failed load must leave the cache empty, so no later request observes a partially configured context. A lock must cover initialization; a plain memoization decorator can evaluate the initializer multiple times on concurrent cache misses and would reintroduce the cost this PR targets. The simple lock shown here also covers cache hits; optimize that only if measurement justifies it and the publication model stays explicit.

Call this factory only for HTTPS, default verification, no request client certificate, and no overriding manager TLS policy, as described in [01_tls_ownership.md](01_tls_ownership.md). HTTP, disabled verification, and explicit custom CA modes bypass the default initializer. Preserve the canonical zip-path helper and the prior helpful missing-path error. Keep any CA parsing failure at the verified-request boundary; do not catch it globally and continue with unexpected roots.

This is a small state model with one empty-to-ready transition, not a generic initialization framework. It preserves once-per-process root loading for the intended concurrent default workload while avoiding a new import dependency. It moves the cost to the first eligible request; that is an intentional tradeoff to preserve caller choice. If eager warm-up is desired for profiling, it can be explicit internal startup work rather than an unavoidable import side effect.

## Suggested regression checks

Add focused tests proving that HTTP, `verify=False`, and an explicit custom bundle do not initialize the default context even when its bundle is missing. A default-verified HTTPS request should retain the expected missing-path error. Verify archive-member extraction still occurs, and test that multiple concurrent first callers receive the same successfully loaded object with only one load.

Also test initialization failure followed by repair: no failed or partial object should remain published. Couple those checks with the identity-free eligibility and explicit-manager-context checks, so future cache expansion cannot silently pull unrelated policies into this lifecycle.

These are remediation suggestions, not claims that the proposed implementation has been executed. No change or test was added to the clone.
