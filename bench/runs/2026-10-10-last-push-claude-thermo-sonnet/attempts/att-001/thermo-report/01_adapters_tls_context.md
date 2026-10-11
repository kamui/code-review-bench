# Detail: `src/requests/adapters.py` TLS context handling

## Measurements and commands

- `git diff main...review-head` shows one file, +28/-18. `wc -l src/requests/adapters.py` gives 616 lines, so the 1k-line rule is not triggered.
- Probe (run with `PYTHONPATH=src` and the provided virtualenv): calling `_urllib3_request_context()` with a mock request and `verify=True` returns `pool_kwargs` with `ssl_context is _preloaded_ssl_context` both for `cert=None` (keys: `cert_reqs`, `ssl_context`) and for `cert=("c.pem", "k.pem")` (keys: `cert_file`, `cert_reqs`, `key_file`, `ssl_context`). The same object is shared.
- `import requests` on this machine took about 0.09 s wall in total with the context built; the PR description attributes about 0.02 s per `load_verify_locations()` call in the profiled environment, and the import path now pays that once.
- `pytest tests/test_requests.py -k "verify or cert or ssl" -x` stopped at `test_pyopenssl_redirect`, a known failure at the merge-base as well (fixture certificate mismatch), so it gave no signal on the change.

## Finding 1 worked evidence: client cert written into the global context

urllib3 2.8.0, `connection._ssl_wrap_socket_and_match_hostname`: when `ssl_context` is not `None`, `context = ssl_context`, then `context.verify_mode = resolve_cert_reqs(cert_reqs)`, then `ssl_wrap_socket(..., certfile=cert_file, keyfile=key_file, ssl_context=context)`. In `util/ssl_.py`, `ssl_wrap_socket` does `if certfile: context.load_cert_chain(certfile, keyfile)` and `context.set_alpn_protocols(ALPN_PROTOCOLS)` on that same object. `_urllib3_request_context()` places `cert_file`/`key_file` in the same `pool_kwargs` as `ssl_context`. Also note the pool key includes `key_ssl_context`, so all `verify=True` pools key on the shared object, which means pool reuse across different client certs is decided by `key_cert_file`, while the underlying context accumulates all chains.

Impact: client identity of one request can be presented on later requests to unrelated hosts; mutation of a shared object after it is in use is what tiran warned against in the PR thread.

### Code-judo proposal

```python
@functools.lru_cache(maxsize=None)
def _default_ssl_context():
    ctx = create_urllib3_context()
    ctx.load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))
    return ctx

def _ca_pool_kwargs(verify, client_cert):
    if verify is False:
        return {"cert_reqs": "CERT_NONE"}
    kwargs = {"cert_reqs": "CERT_REQUIRED"}
    if verify is True:
        if client_cert is None:          # shared context must never see load_cert_chain
            kwargs["ssl_context"] = _default_ssl_context()
    elif isinstance(verify, str):
        kwargs["ca_cert_dir" if os.path.isdir(verify) else "ca_certs"] = verify
    return kwargs
```

When `client_cert` is set with `verify=True`, urllib3 falls back to its own per-pool context (one slow load per pool, which is the pre-PR behaviour) or the factory can return a shallow, per-pool context.

## Finding 2 worked evidence: duplicated decision

Head of `_urllib3_request_context()` (lines 91-100) and `cert_verify()` (lines 297-317) both branch on `os.path.isdir(verify)`; for string `verify` the same path reaches urllib3 via pool kwargs and via `conn.ca_certs`/`conn.ca_cert_dir`. Single-owner proposal: the helper above also raises `OSError("Could not find a suitable TLS CA certificate bundle, invalid path: ...")` when a string path does not exist. `cert_verify()` then becomes:

```python
if url.lower().startswith("https") and verify:
    conn.cert_reqs = "CERT_REQUIRED"
else:
    conn.cert_reqs = "CERT_NONE"
    conn.ca_certs = None
    conn.ca_cert_dir = None
# client cert handling unchanged
```

Net effect is fewer branches than before the PR, not more.

## Finding 3: import-time work

`adapters.py:75-78` runs at import. Consequences: cost for `verify=False` users; zip-extraction side effect at import; failure mode changed from request-time `OSError` with a helpful message to import-time exception. Remedy is the lazily cached factory above.

## Finding 4: implicit contract

`cert_verify()` for `verify=True` no longer sets any CA attribute; correctness depends on `_get_connection()` having been used. A connection from a pool built without the preloaded context ends with `ca_certs=None`, `ca_cert_dir=None`, `cert_reqs=CERT_REQUIRED`, so urllib3 calls `context.load_default_certs()`, the OS trust store. Not exercised by the stock `send()` path, hence lower severity; reported as a contract/boundary smell.

## Finding 5 and 6

See summary. Suggested tests: pool reuse via `adapter._get_connection(...)` identity; `pool.conn_kw["ssl_context"]` is the shared context only for `verify=True` without a client cert; a `cert=` request leaves the shared context's cert chain untouched; a missing default bundle raises the friendly `OSError`.
