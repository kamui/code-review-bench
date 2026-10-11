# Default context lifecycle

## Finding: Default CA initialization runs during every `requests.adapters` import

**Evidence.** The changed code at `src/requests/adapters.py:75-78` creates `_preloaded_ssl_context` at module scope and immediately invokes `load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))`. The new `verify is True` branch at lines 94-95 then injects that shared context into urllib3 pool settings. Request dispatch reaches `cert_verify()` later, at line 536, but the default CA work has already happened simply because `requests.adapters` was imported.

Before this change, `cert_verify()` resolved the default bundle and checked that the path existed only when a request used HTTPS with verification enabled. The changed flow moves that work earlier than the request boundary and removes the prior explicit `OSError` with the invalid bundle path from that code path. A failed `load_verify_locations()` now escapes module import. This affects applications importing Requests for HTTP-only work, `verify=False`, or merely to use unrelated APIs, even though none of those cases need the default trust store.

**Why this is a structural issue.** The optimization correctly identifies that the trust store need not be loaded once per connection, but module import is broader than the feature boundary that needs it. It couples every Requests import to an OpenSSL operation and certifi's availability/readability. The process-wide context is a useful cache; eager construction is not required to achieve one-time reuse.

**Worked code-judo proposal.** Keep a module-level cache slot and lock, and move construction/loading behind a private getter. In pseudocode:

```python
_default_ssl_context = None
_default_ssl_context_lock = threading.Lock()


def _get_default_ssl_context():
    global _default_ssl_context
    if _default_ssl_context is None:
        with _default_ssl_context_lock:
            if _default_ssl_context is None:
                context = create_urllib3_context()
                context.load_verify_locations(
                    extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
                )
                _default_ssl_context = context
    return _default_ssl_context
```

Then `_urllib3_request_context()` calls `_get_default_ssl_context()` only when `verify is True`. Building the local context before assigning it preserves atomic publication: if loading fails, the shared slot stays empty and later verified requests get the failure at the point where the trust store is needed. Once successfully published, every adapter and concurrent request reuses the same unmodified context. The existing custom bundle and disabled-verification branches remain separate and do not initialize this default context.

The key structural change is not adding another TLS mode; it is placing the existing shared-resource lifetime behind the request path that owns its need. If preserving first-request latency is essential, an explicit opt-in warm-up hook could call the same getter, without making all imports perform TLS setup.

## Verification status

No tests were run. The review traced the changed initialization and call flow in `src/requests/adapters.py` and inspected the existing `extract_zipped_paths()` behavior in `src/requests/utils.py`. The checkout was read-only and remained unchanged. Recommended coverage for a follow-up: import with an unavailable default bundle should remain possible; the first verified HTTPS setup should initialize once; concurrent first use should publish one usable context; custom CA and `verify=False` paths should not initialize it.
