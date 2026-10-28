# Thermo nuclear review: psf/requests#6667 (`src/requests/adapters.py`)

Range reviewed: `main...review-head`. One file changed. No tests were added.

## Finding 1 (High): the module-global `_preloaded_ssl_context` overrides any `ssl_context` a custom adapter supplies, for every default `verify=True` request

**Where:** `src/requests/adapters.py:94-95`, in `_urllib3_request_context`. It is called from `_get_connection` at lines 383-401.

**Evidence traced:**
- With `verify is True`, `_urllib3_request_context` now always returns `pool_kwargs["ssl_context"] = _preloaded_ssl_context`.
- `_get_connection` passes that dict as `pool_kwargs=` to `poolmanager.connection_from_host`, or to the proxy manager's `connection_from_host`.
- urllib3's `_merge_pool_kwargs` copies `self.connection_pool_kw` and then lets the per-request override keys win.
- A common requests pattern is to subclass `HTTPAdapter` and pass `ssl_context=...` into `init_poolmanager` or `proxy_manager_for`. It is used to set custom ciphers or a lower security level, a minimum TLS version, a legacy-renegotiation flag, or a private CA loaded into the context.
- Before this PR, `pool_kwargs` carried no `ssl_context`, so the adapter's context was honored.
- I reproduced this offline with an adapter that sets `k["ssl_context"] = ssl.create_default_context()`. `connection_from_host(**host_params, pool_kwargs=kw)` returns a pool whose `conn_kw["ssl_context"]` is `_preloaded_ssl_context`, and not the adapter's context ("user ctx honored: False").

**Failure caused:**
- The adapter's TLS configuration is silently ignored on the default path (`verify=True`, which is the default).
- Connections to servers that need the custom context, such as legacy ciphers or an internal CA, start failing with handshake or verify errors.
- A context that was deliberately hardened, for example a pinned minimum version or restricted ciphers, is silently replaced by the library default. That is a security downgrade.
- The same applies to proxy managers.
- The fix should not override an `ssl_context` that is already configured on the pool manager. The PR text says nothing about this compatibility change.

## Finding 2 (High): the single shared context is mutated by urllib3 on every connection, so client certificates and other per-connection TLS state leak across sessions and hosts

**Where:** `src/requests/adapters.py:75-78` (the global context) and `94-95`.

**Evidence traced:**
- urllib3 2.x, `connection.py::_ssl_wrap_socket_and_match_hostname`, uses a passed context directly (`context = ssl_context`). It then calls `context.verify_mode = ...`, and `context.check_hostname = False` when `assert_hostname` is set or pyOpenSSL is in use.
- `util/ssl_.py::ssl_wrap_socket` calls `context.load_cert_chain(certfile, keyfile)` on that same object whenever `cert_file` is set. `set_alpn_protocols` is also called on it.
- `_urllib3_request_context` puts `cert_file` and `key_file` into the same `pool_kwargs` as `ssl_context`. The pool key includes `cert_file`, so the pools are separate, but they all share one `SSLContext` object.
- Before this PR, urllib3 built a fresh context per pool, so a client certificate stayed inside one pool.

**Failure caused:**
- A request made with `cert=("a.crt", "a.key")` and `verify=True` loads A's identity into the process-wide context.
- A later request in the same process, even from another `Session` or another tenant with no `cert=` and `verify=True`, uses the same context.
- If the other server asks for a client certificate, the TLS handshake can present A's certificate. That is cross-request credential leakage.
- `load_cert_chain` calls from threads that use different certificates race on the shared object, so the identity presented is non-deterministic.
- The PR discussion cited tiran's statement that a shared `SSLContext` is safe as long as it is not reconfigured after use. urllib3 reconfigures it on every connection that sets a client cert or hostname options, and that pattern was not considered.
- A fix would keep the cached context only when no client cert is in use, or would copy it per cert configuration.

## Finding 3 (Medium): import-time certificate loading makes `import requests` fail (or become slow and side-effecting) when the CA bundle is unavailable, and the error message the code used to give is gone

**Where:** `src/requests/adapters.py:75-78`, plus the `cert_verify` change at lines 297-317.

**Evidence traced:**
- The module now runs `create_urllib3_context()` and `load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))` at import time. `requests/__init__.py` imports `adapters` through `sessions`.
- Before this PR, `cert_verify` checked `os.path.exists(cert_loc)` lazily and raised `OSError("Could not find a suitable TLS CA certificate bundle...")` only when a verified request was made. That check was removed for `verify=True`.
- Distro-patched or stripped `certifi` installs that point at a missing bundle, frozen apps, and code that uses only `verify=False` or plain HTTP all pay this cost at import. For the first group the import itself now raises a bare `FileNotFoundError` or `ssl.SSLError`.
- `extract_zipped_paths` can also extract the bundle to a temp directory at import time. The cost is paid by everyone, even if they never make an HTTPS request.

**Failure caused:**
- `import requests` can raise for users who never needed TLS verification, and it raises without the previous diagnostic text.
- Import latency increases for every process.
- Loading lazily, on first use and behind a lock, would keep the performance goal without this regression.

## Finding 4 (Low): `verify=True` correctness now depends on trust state frozen at import time

**Where:** `src/requests/adapters.py:75-78`.

**Evidence traced:**
- The default CA bundle is read once, when the module is imported, and is never refreshed.
- A long-running process that later has `certifi` updated, or that modifies the file at `DEFAULT_CA_BUNDLE_PATH` or the path `extract_zipped_paths` returned, keeps trusting the stale set. Before this PR, the bundle was re-read when a new pool was created.
- No test covers the new behavior: `verify=True` with a custom `ssl_context` or `cert=`, directory `verify`, or the missing-bundle case.
