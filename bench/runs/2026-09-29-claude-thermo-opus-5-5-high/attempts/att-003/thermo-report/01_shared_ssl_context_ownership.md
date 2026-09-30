# 01 — Ownership of the shared `_preloaded_ssl_context`

Scope: `src/requests/adapters.py` at `review-head` (4089f3dc), lines 75–78 (module-global context), 94–95 (injection into `pool_kwargs`), 377–404 (`_get_connection`), 221 (`init_poolmanager`). urllib3 under test: 2.8.0 from the provisioned venv.

## What the diff does

The PR builds one process-wide `SSLContext` at import time, loads the certifi bundle into it, and injects it into every `verify=True` request by writing `pool_kwargs["ssl_context"] = _preloaded_ssl_context` in `_urllib3_request_context`. The premise, from the PR discussion, is that "`SSLContext` is thread safe as long as you don't reconfigure it once it is used by a connection."

The problem is that requests does not control whether the context gets reconfigured. urllib3 does, and it writes to any caller-supplied context on every connect.

## Evidence: urllib3 writes to the context it is handed

`urllib3/connection.py` (`_ssl_wrap_socket_and_match_hostname`, 2.8.0):

```python
    else:
        context = ssl_context

    context.verify_mode = resolve_cert_reqs(cert_reqs)          # line 1042

    if (assert_fingerprint or assert_hostname or assert_hostname is False
        or ssl_.IS_PYOPENSSL or not ssl_.HAS_NEVER_CHECK_COMMON_NAME):
        context.check_hostname = False                            # line 1056
```

`urllib3/util/ssl_.py` (`ssl_wrap_socket`):

```python
    if certfile:
        if key_password is None:
            context.load_cert_chain(certfile, keyfile)            # line 431
```

So these three write paths all land on the module global:

1. **Client certificates (`cert=`)**. `_urllib3_request_context` puts `cert_file` and `key_file` into the same `pool_kwargs` that carry the shared context, and `cert_verify` also sets them on the pool. On connect, urllib3 calls `load_cert_chain` on the shared context. After that, every later `verify=True` connection in the process to any host, on any Session or adapter, presents that client certificate. That is a real disclosure hazard, not just a thread-safety question.
2. **`assert_hostname` / `assert_fingerprint`**. Adapters that set these (for example host-header style adapters) make urllib3 set `check_hostname = False` on the shared context. That turns off OpenSSL-level hostname checking for every other pool that shares the object.
3. **`verify_mode`** gets rewritten on every connect. This is harmless today only because the shared context is never used with `CERT_NONE`. That invariant is implicit and nothing protects it.

## Verification (CONFIRMED, local, no network)

Script `tmp/probe.py` (attempt-private temp dir), run as `PYTHONPATH=clone/src clone-cache/venv/bin/python tmp/probe.py`. It binds a local TCP listener that accepts and closes, so the TLS handshake fails *after* the context has been set up. It also wraps `load_cert_chain` on the global context to record calls:

```
pool uses module-global context: True
connect error (expected): SSLEOFError
load_cert_chain calls on shared global ctx: [('clone/tests/certs/mtls/client/client.pem', 'clone/tests/certs/mtls/client/client.key')]
check_hostname before: True
connect2 error: SSLEOFError
check_hostname after on shared global ctx: False
```

Both writes land on `requests.adapters._preloaded_ssl_context` itself, not on a copy.

## Second ownership defect: the per-request context overrides the adapter's context

`_get_connection` calls `self.poolmanager.connection_from_host(**host_params, pool_kwargs=pool_kwargs)`. urllib3's `PoolManager._merge_pool_kwargs` starts from `self.connection_pool_kw` and lets the per-request override win. Subclassing `HTTPAdapter` and passing `ssl_context=` in `init_poolmanager` is the long-standing, widely documented way to customise TLS in requests (custom ciphers, TLS versions, truststore contexts, mTLS). Once the PR writes `ssl_context` per request, that customisation is silently replaced for every `verify=True` request.

Verification (CONFIRMED), `tmp/probe3.py` against the merge-base source extracted with `git archive main src` and against the head:

```
== base/src
custom adapter ssl_context honored: True
== clone/src
custom adapter ssl_context honored: False
```

## Why this is a structural problem, not only a bug

The design puts a **process-global mutable resource** at the lowest layer (a module function that should just translate `verify`/`cert` into pool kwargs). It then relies on a convention ("don't reconfigure it") that the layer below (urllib3) breaks as part of its normal operation, and that the layer above (adapter subclasses) has always been allowed to override. The leading underscore handles "users shouldn't poke it". It does nothing about the fact that requests' own dependency pokes it.

## Worked code-judo proposal

Move the decision "may this request use the shared default context?" to the one place that has all the facts: the adapter, in `_get_connection`. It knows the pool manager's configured kwargs and the request's client cert. Then make the shared context something that is only handed out when nothing will write to it:

```python
_DEFAULT_SSL_CONTEXT = None

def _default_ssl_context():
    """Process-wide context with the certifi bundle loaded. Never handed to a
    pool that could reconfigure it (client certs, adapter-supplied contexts)."""
    global _DEFAULT_SSL_CONTEXT
    if _DEFAULT_SSL_CONTEXT is None:
        ctx = create_urllib3_context()
        ctx.load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))
        _DEFAULT_SSL_CONTEXT = ctx
    return _DEFAULT_SSL_CONTEXT


def _urllib3_request_context(request, verify, client_cert, poolmanager):
    ...
    can_share_default = (
        verify is True
        and client_cert is None
        and "ssl_context" not in poolmanager.connection_pool_kw
    )
    if can_share_default:
        pool_kwargs["ssl_context"] = _default_ssl_context()
```

(Races on the lazy initialisation only cost a duplicate load. Use `functools.lru_cache(maxsize=1)` if a one-liner is preferred.)

This keeps the performance win for the common case: plain `requests.get(url)`, no client cert, stock adapter. It also returns every other case exactly to the merge-base behaviour, so no new mode is introduced. `assert_hostname` subclasses still share the context and still write to it. The robust fix for those is the same rule ("adapters that customise TLS don't get the shared context"). The adapter can express that with a single class attribute (e.g. `_use_default_ssl_context = True`) instead of requests trying to guess every urllib3 kwarg that mutates.

A more ambitious alternative would be to cache one context per `(ca_location, cert_file, key_file)` tuple, load the client cert ourselves, and stop passing `cert_file`/`key_file` to urllib3. That would also speed up the `verify="/path"` and mTLS cases, which the PR author notes still pay the cost. It is only safe if `cert_verify` also stops writing `conn.cert_file`, and that is a public subclassing hook. So it is a larger change that should come in its own PR, not a quiet addition to this one.

## Test gap

The PR adds no tests. The author wrote that they "couldn't find a way to access such low-level information". The probe shows the information is one attribute away: `adapter._get_connection(prepared, verify, cert=...)` returns the pool, and `pool.conn_kw["ssl_context"]` is the context identity. The minimum regression set: (a) `verify=True` and no cert gives the shared context; (b) `verify=True` plus `cert=` does **not** give the shared context; (c) an adapter whose `init_poolmanager` supplies `ssl_context` keeps it; (d) `verify=False` and `verify="/path"` do not get the shared context.
