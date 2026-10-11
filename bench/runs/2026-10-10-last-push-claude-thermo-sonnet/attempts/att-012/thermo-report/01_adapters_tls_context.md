# 01 — `src/requests/adapters.py` TLS context handling

Scope: `git diff main...review-head`, one file, +28/−18. File length at head: 616 lines (no 1k-line crossing).

## Commands and observations

- `git diff main...review-head` — shows the module-level `_preloaded_ssl_context`, the new `verify is True` / `str` branches in `_urllib3_request_context`, and the slimmed `cert_verify`.
- `grep -n` on head: context built at lines 75-78; used at 94-95; directory branch at 96-101; `cert_verify` at 285, `verify is not True` branch at 304, `ca_cert_dir` assignment at 317.
- Read installed urllib3 2.8.0 (`urllib3/connection.py` around line 1030, `urllib3/util/ssl_.py` around lines 405-435).

## Evidence for F1 (shared context is mutated)

urllib3 `_ssl_wrap_socket_and_match_hostname`: when `ssl_context` is supplied it does `context = ssl_context`, then unconditionally `context.verify_mode = resolve_cert_reqs(cert_reqs)`, and may set `context.check_hostname = False` for fingerprint or hostname-assertion cases. `ssl_wrap_socket`: `if certfile: context.load_cert_chain(certfile, keyfile[, key_password])`, and `context.set_alpn_protocols(...)` every time. `_urllib3_request_context` still places `cert_file`/`key_file` in the same `pool_kwargs` that now also carries `ssl_context=_preloaded_ssl_context`. Hence client-cert material is loaded into the single process-wide context. SSLContext cert chains are additive, so a later connection with no client cert still has the chain loaded and may present it when a server requests one. Pool keys include `cert_file` but not a per-pool context, so isolation by pool does not apply.

Verification status: established by source reading. Not reproduced end to end (no network; handshake test would need a local mTLS fixture, which the allowance does not cover for this review).

Worked proposal: replace the global with

```python
@functools.lru_cache(maxsize=None)
def _default_ssl_context():
    ctx = create_urllib3_context()
    ctx.load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))
    return ctx
```

and in `_urllib3_request_context` only set `pool_kwargs["ssl_context"]` when `client_cert is None`. When a client cert is supplied the old behavior (urllib3 builds a per-pool context and loads default CAs via `ca_certs`) is retained for correctness; that path is rarer and its cost is paid once per pool.

## Evidence for F2 (import-time work)

Lines 75-78 execute at module import. `extract_zipped_paths` (utils.py:268) can extract to the temp dir for zipped installs. The removed `cert_verify` code raised `OSError("Could not find a suitable TLS CA certificate bundle, invalid path: ...")` for the default path; the new `verify is True` path has no such check. A missing certifi bundle now fails at import (or as an ssl error inside urllib3 with no requests message).

Worked proposal: lazy cached factory (above); wrap the load in `try/except OSError` and re-raise with the old message at first use.

## Evidence for F3 (duplicated decoding)

```
_urllib3_request_context:  verify is True -> ssl_context
                           str, not dir   -> ca_certs
                           str, dir       -> ca_cert_dir
cert_verify:               verify is not True -> exists check
                                                 not dir -> conn.ca_certs
                                                 dir     -> conn.ca_cert_dir
```

The connection returned by `_get_connection` comes from a pool built with the same `pool_kwargs`, so the `conn.*` assignments restate the same values. `cert_verify` treats any truthy non-True value as a path; `_urllib3_request_context` only treats `str` that way.

Worked code-judo proposal: one helper

```python
def _ca_kwargs(verify):
    if verify is True:
        return {"ssl_context": _default_ssl_context()}
    if isinstance(verify, str):
        if not os.path.exists(verify):
            raise OSError(f"Could not find a suitable TLS CA certificate bundle, invalid path: {verify}")
        key = "ca_cert_dir" if os.path.isdir(verify) else "ca_certs"
        return {key: verify}
    return {}
```

`_urllib3_request_context` merges it; `cert_verify` keeps only `conn.cert_reqs` handling and client-cert handling. This deletes two branches, one comment block, and one `isdir` stat per request.

## Evidence for F4 (hook semantics and tests)

`cert_verify` is a documented subclass hook; after the PR `conn.ca_certs` is no longer set for `verify=True`. No test files changed in the PR. Suggested tests: (a) patch `ssl.SSLContext.load_verify_locations` to count calls across N `adapter.send`-less `_get_connection` + connect simulations with `verify=True`; (b) assert `pool.conn_kw["ca_cert_dir"]` for a directory `verify`; (c) assert `OSError` for a missing path; (d) assert two pools with different client certs do not share a context.

## Not run

No tests were executed; the findings do not depend on test outcomes.
