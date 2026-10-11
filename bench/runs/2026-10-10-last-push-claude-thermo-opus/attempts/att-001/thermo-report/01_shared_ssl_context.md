# 01 - The process-wide SSLContext: ownership, mutation, shadowing, import time

Scope: `src/requests/adapters.py:75-78` (the module-level `_preloaded_ssl_context`) and
`src/requests/adapters.py:92-101` (the `verify is True` branch of `_urllib3_request_context`
that hands it to urllib3). Carries the evidence for summary findings F1, F2 and F4.

Range reviewed: `8dd3b26b..4089f3dc` (`git diff main...review-head`), one file, +28 / -18.
`src/requests/adapters.py` goes from 606 to 616 lines, so there is no file-size concern.

## What the change does

```python
_preloaded_ssl_context = create_urllib3_context()          # adapters.py:75
_preloaded_ssl_context.load_verify_locations(              # adapters.py:76
    extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
)
...
    elif verify is True:                                    # adapters.py:94
        pool_kwargs["ssl_context"] = _preloaded_ssl_context # adapters.py:95
```

One `ssl.SSLContext` is built when `requests.adapters` is imported and is then passed, as a
per-request `pool_kwargs` override, to every pool that serves a `verify=True` request, in every
`Session`, every adapter and every thread of the process.

The performance idea is sound: `load_verify_locations()` is slow and urllib3 repeats it per new
connection when it has to build its own context. The problem is the object model chosen to get
there. The context is treated as an immutable, read-only trust store, but the object it is
handed to does not treat it that way.

## F1 - urllib3 reconfigures whatever context it is given, on every new connection

Evidence, urllib3 2.8.0 as installed in the provided virtualenv:

`urllib3/connection.py:1030-1056`

```python
    if ssl_context is None:
        default_ssl_context = True
        context = create_urllib3_context(ssl_version=..., ssl_minimum_version=...,
                                         ssl_maximum_version=..., cert_reqs=...)
    else:
        context = ssl_context

    context.verify_mode = resolve_cert_reqs(cert_reqs)          # write
    if (assert_fingerprint or assert_hostname or assert_hostname is False or ...):
        context.check_hostname = False                           # write
```

`urllib3/util/ssl_.py:413-435`

```python
    if ca_certs or ca_cert_dir or ca_cert_data:
        context.load_verify_locations(ca_certs, ca_cert_dir, ca_cert_data)   # write
    ...
    if certfile:
        context.load_cert_chain(certfile, keyfile)                           # write
    context.set_alpn_protocols(ALPN_PROTOCOLS)                               # write
```

So the "preloaded" context is written to by every connection that uses it. The write that
matters most is `load_cert_chain`. `_urllib3_request_context` puts `cert_file` / `key_file`
into the same `pool_kwargs` as the shared `ssl_context` (`adapters.py:102-109`), so a request
made with `verify=True, cert=...` installs that client certificate and private key into the
one context every other `verify=True` request in the process uses. `SSLContext` has no API to
unload a certificate chain, so the state is permanent for the life of the process.

This is exactly the condition the packet's own discussion warned about. The CPython ssl
maintainer's comment in the thread says a context is safe to share "as long as you don't
reconfigure it once it is used by a connection" and names "mTLS certs" and "verification
settings" as the changes that "can lead to surprising behavior". The PR shares the context and
leaves the reconfiguration in place.

### Verified by execution

Probe `test_1_client_cert_leaks_into_other_sessions` (scratch file, see
`03_verification.md`). Two independent local TLS servers on different ports, both with
`verify_mode = CERT_OPTIONAL`; each reports whether the peer presented a certificate.

```
before:    {'client_cert_presented': False, 'tls': 'TLSv1.3'}   # fresh Session -> server B
with_cert: {'client_cert_presented': True,  'tls': 'TLSv1.3'}   # other Session -> server A, cert=client.pem
after:     {'client_cert_presented': True,  'tls': 'TLSv1.3'}   # third, brand-new Session -> server B, no cert
plain api: {'client_cert_presented': True,  'tls': 'TLSv1.3'}   # requests.get(server B), no cert
```

A session that never received a `cert=` argument authenticates to a different server with
another session's client certificate. Pool keys do not protect against this: the two requests
land in different pools (`key_cert_file` differs) but both pools hold the same context object.

Probe `test_4_assert_hostname_adapter_mutates_shared_context`: after one request through an
adapter whose `init_poolmanager` passes `assert_hostname="localhost"`,
`requests.adapters._preloaded_ssl_context.check_hostname` is `False` for the rest of the
process. urllib3 compensates with its own hostname match when `check_hostname` is off
(`connection.py:1098-1120`), so I am not claiming a hostname-verification bypass; I am citing
it as a second, independently observed write to state that is supposed to be read-only.

### Not verified by execution (reasoned from the code above)

Concurrency. `load_cert_chain(A)` and the subsequent `wrap_socket` are separate calls on the
shared object and the GIL is released during the file IO, so two threads opening connections
with different client certificates can interleave as load A, load B, wrap A's socket, and A
then authenticates as B. I did not build a race harness. This is the workload the PR is
written for (many concurrent requests), so it deserves a test before the design is accepted.

The repository's own suite already exercises the mutation: `tests/test_requests.py:2962`
calls `s.get(url, cert=cert)` with the default `verify=True`, which loads the mTLS fixture
certificate into the process-wide context for every test that runs afterwards.

## F2 - the injected context shadows adapter-level TLS configuration

`pool_kwargs` given to `PoolManager.connection_from_host` are merged over the manager's own
`connection_pool_kw` (`urllib3/poolmanager.py:390-410`, override wins). At the merge-base the
per-request override only ever contained `cert_reqs`, `ca_certs`, `cert_file` and `key_file`.
The head adds `ssl_context`, which is the one key that changes the meaning of the others:
when urllib3 receives a context it skips `create_urllib3_context(...)` entirely
(`connection.py:1030-1040`), so `ssl_version`, `ssl_minimum_version` and `ssl_maximum_version`
configured on the pool manager are never read, and a context configured on the pool manager is
replaced outright.

Passing these through `init_poolmanager` is the documented extension point.
`docs/user/advanced.rst:1030-1033` teaches exactly this:

```python
        def init_poolmanager(self, connections, maxsize, block=False):
            self.poolmanager = PoolManager(
                num_pools=connections, maxsize=maxsize,
                block=block, ssl_version=ssl.PROTOCOL_SSLv3)
```

### Verified by execution

Probe `test_2_custom_ssl_context_is_replaced_when_verify_true`: an adapter whose
`init_poolmanager` passes `ssl_context=mine`. `adapter._get_connection(req, certifi.where())`
returns a pool with `conn_kw["ssl_context"] is mine`; `adapter._get_connection(req, True)`
returns a pool whose context is `requests.adapters._preloaded_ssl_context`. Custom ciphers,
a private CA loaded into the context, a pinned protocol range, a client certificate loaded
into the context: all dropped, with no error, for the default value of `verify`.

Probe `test_3_tls_version_pin_is_ignored_when_verify_true`: a server capped at TLS 1.2 and an
adapter created with `ssl_minimum_version=ssl.TLSVersion.TLSv1_3`. With `verify=<bundle path>`
the handshake fails with `requests.exceptions.SSLError`, as the pin requires. With
`verify=True` the request succeeds and the server reports `'tls': 'TLSv1.2'`. A security
hardening setting is silently discarded on the default path and honoured on the non-default
one.

The explicit-path control in both probes is the code path the merge-base used for every value
of `verify` (no context injected), which is why I describe this as a regression. I did not
execute the merge-base itself; the base behaviour is established by reading the diff.

### Why this is a design problem and not just a missing `if`

`_urllib3_request_context` is a module-level free function that sees only
`(request, verify, client_cert)`. Whether a shared context may be used depends on facts it
cannot see: what the adapter's pool manager was configured with. The decision is in the wrong
place. Any fix that keeps it there has to smuggle adapter state into a free function; the
natural home is the adapter, which owns the pool manager.

## F4 - the context is built as an import side effect

`adapters.py:75-78` runs on `import requests` (`requests/__init__.py` imports `api`, which
imports `sessions`, which imports `adapters`).

Consequences, with verification status:

- Import now fails where it used to succeed. Verified by execution
  (`test_6_import_time_cost_and_failure`): with `requests.utils.DEFAULT_CA_BUNDLE_PATH`
  pointing at a missing file, executing the module body raises `FileNotFoundError`. At the
  merge-base the same condition produced
  `OSError("Could not find a suitable TLS CA certificate bundle, invalid path: ...")` from
  `cert_verify`, only when a verified HTTPS request was made; the head deletes that message for
  the default bundle (the `if not cert_loc or not os.path.exists(cert_loc)` guard now only
  covers user-supplied paths). A program that only talks plain HTTP, or always passes
  `verify=False` or its own bundle, can no longer import the library in that environment.
- Interpreters built without `ssl`. Verified by code reading only: `urllib3/util/ssl_.py:223-224`
  raises `TypeError("Can't create an SSLContext object without an ssl module")`, and
  `requests/__init__.py:119-124` shows the package explicitly tolerates `import ssl` failing.
  Not executed (no ssl-less interpreter available).
- Zip extraction at import. `extract_zipped_paths` (`requests/utils.py:268`) writes the bundle
  to the temp directory when requests runs from a zip; that now happens on import instead of on
  first verified request. Code reading only.
- The value is a snapshot. Anything that changes after import is ignored for `verify=True`:
  reassigning `DEFAULT_CA_BUNDLE_PATH`, or calling `urllib3.contrib.pyopenssl.inject_into_urllib3()`
  later (the context stays a stdlib `ssl.SSLContext`). Code reading only.
- Cost. Measured in the probe on this machine (CPython 3.13.15, OpenSSL 3.5.8):
  `create_urllib3_context()` plus `load_verify_locations(certifi)` took 3.3, 3.4, 2.9, 2.8 and
  2.7 ms over five runs. That is small here. The PR body's own profile (0.681 s across 30
  calls on OpenSSL 3.0.2) implies roughly 23 ms per call on the author's setup; I did not
  reproduce that figure. Either way it is paid by every importer, including those who never
  open a verified connection.

## Worked proposal (not executed; the clone is read-only)

The move is to stop treating "which context" as a property of the request and make it a
property of what the adapter can prove about the pool: the shared context is used only when
nothing will write to it and nothing is shadowed by it, and it is built on first use.

```python
# Pool-manager keywords that either configure the context urllib3 would build
# or cause urllib3 to write to the context it is handed.
_CONTEXT_SHAPING_KEYWORDS = frozenset(
    (
        "ssl_context",
        "ssl_version",
        "ssl_minimum_version",
        "ssl_maximum_version",
        "assert_hostname",
        "assert_fingerprint",
        "cert_file",
        "key_file",
        "key_password",
    )
)


@functools.lru_cache(maxsize=1)
def _default_ssl_context():
    """Verify-only context with the default CA bundle, built on first use."""
    context = create_urllib3_context()
    context.load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))
    return context
```

and in the adapter, where the pool manager is known:

```python
    def _can_share_context(self, manager, verify, cert):
        return (
            verify is True
            and cert is None
            and _CONTEXT_SHAPING_KEYWORDS.isdisjoint(manager.connection_pool_kw)
        )
```

`_get_connection` resolves the manager first (pool manager or proxy manager), then asks for
the TLS keywords with `share_context=self._can_share_context(manager, verify, cert)`; see
`02_verify_policy.md` for the single function that produces those keywords. When the answer is
no, the request takes the merge-base path (`ca_certs=<default bundle>`), which is slower but is
the behaviour every existing adapter subclass was written against.

What this deletes: the import side effect, the import-time failure modes, the client-cert
write to shared state, and the shadowing of adapter configuration. What it keeps: the entire
speed-up for the case the PR measured (plain `requests.get(url)` / default `Session`).

If the maintainers would rather not carry a keyword allow-list, the more conservative shape is
a context owned by the adapter instance instead of the module, still excluded when a client
certificate is present. That narrows the blast radius from the process to one adapter but does
not by itself fix F2, so I prefer the version above.
