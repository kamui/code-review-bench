# 01 — Shared context state (findings 1, 2, 3)

Subject: `src/requests/adapters.py` at head `4089f3dc`, lines 75-78 (the global), 91-101 (the `verify` branch in `_urllib3_request_context`), 102-109 (client certificate kwargs), 297-321 (`cert_verify`).

Environment for every executed statement: the pre-provisioned virtualenv, urllib3 2.8.0, Python 3.13.15, OpenSSL 3.5.8, `PYTHONPATH=<clone>/src`. Probe source is `probes/test_probe.py`; it was run once, offline:

```
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=$PWD/src <cache>/venv/bin/python -m pytest \
    -p no:cacheprovider -s -q -W ignore <work>/scratch/test_probe.py
...
8 passed in 0.17s
```

Each probe asserts the *observed* head behaviour, so "passed" means "the defect reproduced". An autouse fixture rebuilds `adapters._preloaded_ssl_context` exactly as lines 75-78 do before each probe, so that one probe's mutation of the global cannot produce another probe's result. The merge-base was not executed; statements about it come from `git show main:src/requests/adapters.py`.

## What the diff assumes, and what urllib3 actually does

The design rests on one assumption: a context with the CA bundle already loaded can be handed to urllib3 for every verified request and will simply be used. Reading urllib3 2.8.0 shows three places where a caller-supplied context is written to on each connection.

In `urllib3/connection.py`, `_ssl_wrap_socket_and_match_hostname`:

```python
1039    else:
1040        context = ssl_context
1041
1042    context.verify_mode = resolve_cert_reqs(cert_reqs)
...
1056        context.check_hostname = False      # when assert_hostname / assert_fingerprint / pyOpenSSL
```

In `urllib3/util/ssl_.py`, `ssl_wrap_socket`:

```python
429    if certfile:
430        if key_password is None:
431            context.load_cert_chain(certfile, keyfile)
```

None of these is guarded by "did urllib3 create this context itself". The only thing that *is* guarded that way is the trust-store fallback (`default_ssl_context`, lines 1030-1068). So urllib3's contract for `ssl_context=` is "this is yours, I will configure it for this connection", which is fine when the context belongs to one pool and wrong when the same object is the default for a whole process. The PR discussion established that sharing an `SSLContext` across threads is safe "as long as you don't reconfigure it once it is used by a connection"; the code path chosen reconfigures it on every connection.

## Finding 1 — client certificates leak across sessions

`_urllib3_request_context` produces, for `requests.get(url, cert=(pem, key))` with the default `verify=True`:

```python
pool_kwargs = {"ssl_context": _preloaded_ssl_context,   # line 95
               "cert_reqs": "CERT_REQUIRED",            # line 101
               "cert_file": pem, "key_file": key}       # lines 104-105
```

The pool key differs from the certificate-less pool (urllib3 keys on `cert_file`), so the pools are distinct, but both hold the same context object, and connecting the first one runs `load_cert_chain` on it.

Probe 3 builds a throwaway CA with `trustme`, starts a TLS server on 127.0.0.1 with `verify_mode = CERT_OPTIONAL`, records `getpeercert()` per accepted connection, and issues three requests. The only test-specific step on the client is adding the throwaway CA to the (fixture-rebuilt) preloaded context so that `verify=True` succeeds against localhost.

```python
requests.get(url)                                  # 1: anonymous
requests.get(url, cert=(str(pem), str(key)))       # 2: authenticated
requests.Session().get(url)                        # 3: unrelated session, NO cert
```

Output:

```
PROBE3 server saw client certificate per request: [False, True, True]
PROBE3 third (cert-less, separate Session) peer cert: {'subject': ((('organizationName', 'trustme v1.2.1'),), ...
        'subjectAltName': (('email', 'client@example.test'),)}
```

Request 3 has no `cert`, uses a new `Session`, a new adapter and a new pool, and still authenticates as `client@example.test`. On the merge-base each pool's connections got a context that urllib3 created for that connection, so a certificate could only be presented by a pool that was configured with it.

Consequences worth naming: the identity is presented to *any* host that requests a client certificate during a later `verify=True` handshake, not only the one it was configured for; the private key stays loaded for the life of the process; and with two sessions using different client certificates concurrently there is a window between one thread's `load_cert_chain` and its `wrap_socket` in which the other thread's certificate is the one in the context.

Remedy. The shared context must not be used when `client_cert is not None`. In that case fall back to what the merge-base did (pass the bundle path as `ca_certs`, let urllib3 build a private context for the connection). In the restructuring in `02_trust_anchor_ownership.md` this is one term of a single predicate.

## Finding 2 — adapter-level TLS configuration is overridden

`_get_connection` passes `pool_kwargs` to `connection_from_host` (lines 394-402), and urllib3 merges them over the manager's own keyword arguments:

```python
400        base_pool_kwargs = self.connection_pool_kw.copy()
401        if override:
402            for key, value in override.items():
...
409                    base_pool_kwargs[key] = value
```

So whatever the adapter gave its `PoolManager` under the key `ssl_context` is replaced for every `verify=True` request.

Probe 1:

```python
class CtxAdapter(HTTPAdapter):
    def init_poolmanager(self, *args, **kwargs):
        kwargs["ssl_context"] = custom
        super().init_poolmanager(*args, **kwargs)

pool = CtxAdapter()._get_connection(prepared_https_request, True)
```

```
PROBE1 pool ssl_context is custom: False | is _preloaded: True
```

Probe 2 uses the shape of the documented example (`docs/user/advanced.rst:1030`), passing `ssl_minimum_version=ssl.TLSVersion.TLSv1_3` instead of the obsolete SSLv3 constant:

```
PROBE2 ... context actually handed to urllib3 has minimum_version: 771
```

771 is `TLSVersion.TLSv1_2`. The keyword still reaches the pool, but urllib3 consumes `ssl_version` / `ssl_minimum_version` / `ssl_maximum_version` only inside the `if ssl_context is None:` branch at `connection.py:1031-1038`, so once requests supplies a context those keywords are dead. I did not run an end-to-end handshake for this probe; the conclusion combines the observed context with that branch.

This is a silent downgrade of user configuration on the default code path. `proxy_manager_for(proxy, **proxy_kwargs)` is affected the same way, since the same `pool_kwargs` are passed to the proxy manager at lines 395-397.

Remedy. A default must have lower precedence than explicit configuration. Two workable forms: (a) in the resolver, consult the manager that will serve the request and skip the shared context if its `connection_pool_kw` already contains any TLS-shaping key; (b) install the default at manager construction (`init_poolmanager` / `proxy_manager_for`) with `setdefault`, so that subclasses win by ordinary keyword precedence. Form (b) reads more naturally but needs care, because adding `ca_certs` on top of a manager-level shared context for `verify="/path"` would load the custom bundle *into* the shared context; form (a) keeps the decision in one function and is the one sketched in detail file 02.

## Finding 3 — HTTP through an HTTPS proxy raises `ValueError`

Trace for `requests.get("http://example.invalid/", proxies={"http": "https://127.0.0.1:P"})`:

1. `_urllib3_request_context` (lines 91-101): `verify is True`, so `pool_kwargs = {"ssl_context": <shared>, "cert_reqs": "CERT_REQUIRED"}`. The request scheme is not consulted.
2. `ProxyManager.connection_from_host` (`poolmanager.py:628-641`): scheme is `http`, so the pool is built for `self.proxy.scheme == "https"` with those kwargs. It is an `HTTPSConnectionPool` to the proxy holding the shared context.
3. `cert_verify` (line 297): the URL does not start with `https`, so the `else` branch (lines 318-321) sets `conn.cert_reqs = "CERT_NONE"` on that pool.
4. `urlopen` → `connect` → `connection.py:1042`: `context.verify_mode = CERT_NONE` on the shared context, whose `check_hostname` is `True`.

Probe 4, with a bare listening socket standing in for the proxy:

```
PROBE4 http-over-https-proxy raised: builtins ValueError - Cannot set verify_mode to CERT_NONE when check_hostname is enabled.
```

The exception type is the builtin; none of the handlers at `adapters.py:578-614` covers it, so callers catching `requests.exceptions.RequestException` do not catch this.

On the merge-base step 1 produced no `ssl_context`, so at step 4 urllib3 created its own context with `cert_reqs=CERT_NONE` (`create_urllib3_context` clears `check_hostname` before setting the mode, `util/ssl_.py:316-317`) and proceeded to the handshake. That the proxy was not verified there is a pre-existing weakness and is not introduced by this PR; what the PR introduces is the crash. Note also the near miss: the only reason the process-wide context is not switched to `CERT_NONE` here is that CPython rejects the assignment. If any earlier connection had cleared `check_hostname` on the shared context (line 1056 does so for pools configured with `assert_hostname` or `assert_fingerprint`), the assignment would be accepted.

Remedy. Do not attach the shared context unless the request URL is HTTPS. More fundamentally, `cert_reqs` for a pool should be decided once: today `_urllib3_request_context` derives it from `verify` and `cert_verify` re-derives it from the URL prefix and overwrites it on the pool.

## Not a finding, recorded for completeness

`requests/__init__.py:126-129` injects pyOpenSSL before `adapters` is imported, so the import-time context has the right class in that case. A caller who injects *after* importing requests keeps a stdlib context for `verify=True` requests. This is read from the code and was not executed; it appears as an open question in the summary.
