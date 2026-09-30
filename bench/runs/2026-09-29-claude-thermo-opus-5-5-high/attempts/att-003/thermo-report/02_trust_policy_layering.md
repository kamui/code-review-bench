# 02 — Trust-store policy split across two layers

Scope: `src/requests/adapters.py` at `review-head`: `_urllib3_request_context` lines 91–101 and `HTTPAdapter.cert_verify` lines 285–320. `send()` calls them in sequence at lines 532 and 536.

## What the code looks like after the PR

The meaning of `verify` is now decided twice, with different type predicates, in two layers that run one after the other on the same pool.

`_urllib3_request_context` (runs first, via `_get_connection`):

```python
    cert_reqs = "CERT_REQUIRED"
    if verify is False:
        cert_reqs = "CERT_NONE"
    elif verify is True:
        pool_kwargs["ssl_context"] = _preloaded_ssl_context
    elif isinstance(verify, str):
        if not os.path.isdir(verify):
            pool_kwargs["ca_certs"] = verify
        else:
            pool_kwargs["ca_cert_dir"] = verify
    pool_kwargs["cert_reqs"] = cert_reqs
```

`cert_verify` (runs second, on the pool the first function just selected):

```python
        if url.lower().startswith("https") and verify:
            conn.cert_reqs = "CERT_REQUIRED"
            # Only load the CA certificates if 'verify' is a string ...
            if verify is not True:
                # `verify` must be a str with a path then
                cert_loc = verify
                if not os.path.exists(cert_loc):
                    raise OSError(...)
                if not os.path.isdir(cert_loc):
                    conn.ca_certs = cert_loc
                else:
                    conn.ca_cert_dir = cert_loc
```

## Problems

1. **Copy-pasted dispatch.** The `isdir → ca_cert_dir / else → ca_certs` fork now exists in both functions. For a `str` path, `cert_verify` writes the same value onto a pool that was already built with it, so its branch does nothing new. The author's own review reply says so: "I believe this is now fully redundant with the corresponding logic in `cert_verify()`." The redundancy was acknowledged and kept anyway.

2. **The two predicates disagree, and the comment is wrong.** `_urllib3_request_context` handles only `isinstance(verify, str)`. `cert_verify` handles `verify is not True`, i.e. any truthy value. For a truthy non-`str` such as a `pathlib.Path` (it works because of `os.path.exists`), the first layer does nothing and the second layer is the *only* thing configuring trust. So the comment "`verify` must be a str with a path then" is false. The real invariant is "whichever of the two layers recognises the type wins", and a reader can only work that out by reading both functions together. It also means such a request gets a pool keyed without `ca_certs` and then has `ca_certs` written onto it after the fact, which is the pool-key/pool-state mismatch that the previous `_get_connection` change was meant to remove.

3. **The `verify is True` path now depends on the first layer having run.** `cert_verify` deliberately skips setting any trust location for `verify=True` and relies on the pool already carrying `_preloaded_ssl_context`. The long comment in `cert_verify` explains this cross-function coupling instead of removing it. A pool that reaches `cert_verify` through any other route (e.g. a subclass that builds its pool differently, or the adapter-supplied-context case in `01`) now verifies against urllib3's `load_default_certs()` system store instead of certifi. That is a silent change of trust root, not an error.

4. **The actionable error for a missing default bundle was deleted.** The merge-base raised `OSError("Could not find a suitable TLS CA certificate bundle, invalid path: …")` when the certifi path was missing. The PR removes that branch. The failure now happens at import time with a bare `FileNotFoundError` (see `03`).

## Verification

Items 1, 2 and 4 are CONFIRMED by reading the head source (line anchors above) and by the base/head probe in `03`. Item 3 is CONFIRMED by reading urllib3 2.8.0 `connection.py` lines 1060–1068: `load_default_certs()` is called when there is no `ca_certs`/`ca_cert_dir`/`ca_cert_data` and urllib3 built the context itself. Together with the adapter-context override probe in `01`, this shows the dependency is real.

## Worked code-judo proposal

Make `_urllib3_request_context` the single owner of "what does `verify` mean". Then reduce `cert_verify`'s trust half to validation plus the legacy attribute writes that subclasses might rely on.

```python
def _trust_kwargs(verify, client_cert, poolmanager):
    if verify is False:
        return {"cert_reqs": "CERT_NONE"}
    if verify is True:
        if client_cert is None and "ssl_context" not in poolmanager.connection_pool_kw:
            return {"cert_reqs": "CERT_REQUIRED", "ssl_context": _default_ssl_context()}
        verify = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
    verify = os.fspath(verify)
    if not os.path.exists(verify):
        raise OSError(
            f"Could not find a suitable TLS CA certificate bundle, invalid path: {verify}"
        )
    key = "ca_cert_dir" if os.path.isdir(verify) else "ca_certs"
    return {"cert_reqs": "CERT_REQUIRED", key: verify}
```

`_urllib3_request_context` then does `pool_kwargs.update(_trust_kwargs(...))`. The pool key and pool state agree for every `verify` shape, including `Path`. The missing-bundle error comes back and is raised at request time for both the default and custom paths. `cert_verify` keeps its public signature for subclasses but no longer needs its own `isdir` fork or the paragraph-long comment explaining why it sometimes does nothing. It can keep writing the same attributes derived from `_trust_kwargs` for compatibility, or become a no-op for trust if the maintainers accept that. The result is one decision table instead of two partially overlapping ones.
