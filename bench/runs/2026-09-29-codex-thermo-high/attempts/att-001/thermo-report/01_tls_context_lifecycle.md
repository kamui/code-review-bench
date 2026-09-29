# TLS context lifecycle

## Finding: Keep client certificates out of the shared default TLS context

In `src/requests/adapters.py:75-78`, import-time initialization creates one `_preloaded_ssl_context` and loads the default CA bundle into it. At line 95, every `verify=True` request receives that same object as `pool_kwargs["ssl_context"]`. The same `_urllib3_request_context` adds request-specific client certificate paths to `pool_kwargs` at lines 102-109. `HTTPAdapter.send` obtains the pool at lines 532-536, then calls `conn.urlopen`; urllib3 uses the context supplied to the connection pool while wrapping the TLS socket.

The provisioned urllib3 2.8.0 implementation was inspected directly with `inspect.getsource(urllib3.util.ssl_.ssl_wrap_socket)`. It assigns the passed `ssl_context` to `context`, then calls `context.load_cert_chain(certfile, keyfile)` when `certfile` is set. This mutates the exact shared context. A later request using that context retains the previously loaded client identity even if it specifies no client certificate; a different client certificate can overwrite it. Concurrent handshakes can also observe shared context mutation. Pool-key differences for certificate paths do not isolate the underlying context object, because all those pools are passed the same module-level context.

This behavior is introduced by the diff: before the shared context was passed, urllib3 could create a request/pool-specific context for the connection and load that request's client certificate there. The change turns a performance cache for trust roots into a shared mutable store for both trust roots and per-request identity. That is a boundary problem as well as a TLS correctness and isolation bug.

## Worked code-judo proposal

Keep the module-level preloaded context solely for requests with the default trust store and no client certificate. For a request with `client_cert`, choose a distinct context whose lifecycle is tied to that certificate/key identity; load the default CA bundle into that context once, then ensure only pools for that same identity receive it. Another valid shape is to centralize context construction behind a small adapter-owned cache keyed by the effective trust configuration and client identity, so the cache owns complete immutable request configuration rather than sharing one mutable context across unrelated identities. Do not load a client certificate into the default singleton and try to clean it afterward: `SSLContext` has no operation to remove the loaded identity, and mutation would remain unsafe under concurrency.

The simpler correctness boundary is explicit: a context may be shared only when all settings mutated during connection setup are the same for every user of that context. Default verification meets that boundary only when no client certificate is loaded; mTLS requires a context isolated by identity.

## Verification status and evidence

Static inspection only; no test suite was run. Evidence commands:

- `git diff --find-renames --find-copies --unified=80 main...review-head -- src/requests/adapters.py` — shows the singleton creation, its assignment for `verify=True`, and client-certificate pool kwargs.
- `nl -ba src/requests/adapters.py | sed -n '66,106p;510,545p'` — confirms the relevant source locations and request flow.
- `/home/jack/.t3/bench-runs/2026-09-29-codex-thermo-high/att-001/clone-cache/venv/bin/python` with `inspect.getsource(urllib3.util.ssl_.ssl_wrap_socket)` — confirms that a supplied context receives `load_cert_chain` calls when `certfile` is provided.

Regression coverage should exercise two sequential requests with different client certificates and a no-certificate request after mTLS. A concurrent case should verify that each server sees only the certificate configured for its request. The review did not execute these tests.
