# Thermo-nuclear review: psf/requests#6667

Range: `8dd3b26b..4089f3dc`. One file changed, `src/requests/adapters.py`.

## Finding 1 (High): client certificates are loaded into one process-wide shared SSLContext

**Files:** `src/requests/adapters.py` lines 75-78 (module-level `_preloaded_ssl_context`) and lines 94-95 (`pool_kwargs["ssl_context"] = _preloaded_ssl_context`). The client-cert handling in `_urllib3_request_context` (lines 101-108) interacts with it.

**Evidence traced:**
- With `verify=True`, every HTTPS connection pool, for every Session, adapter and thread, now gets the same module-global `SSLContext`.
- When the caller also passes `cert=` (or `session.cert`), `_urllib3_request_context` adds `cert_file` and `key_file` to the same `pool_kwargs`.
- In the installed urllib3 2.8.0, `ssl_wrap_socket` (`util/ssl_.py` ~L430) calls `context.load_cert_chain(certfile, keyfile)` on the context it is given. It does this on every new connection.
- `load_cert_chain` mutates the context. It does not scope the certificate to one connection.
- Before this PR, urllib3 built a context per pool, so a client cert never left that pool. Now it is written into the context shared by everything.

**Failure:**
1. After one `requests.get(url_a, cert=(A.pem, A.key))`, the shared context holds identity A.
2. A later `requests.get(url_b)` with `verify=True` and no client cert, anywhere in the process, uses that same context. If server B sends a CertificateRequest, the handshake presents identity A. The client certificate is disclosed to unrelated hosts.
3. Two threads using different client certs race on `load_cert_chain`. Thread 1's handshake can present thread 2's identity. That is a mix-up of mTLS identities, with possible wrong-identity authentication and authorization.
4. Pool keys include `cert_file` and `key_file`, so separate pools exist, but all of them share the one mutable context.

The same shared-mutation concern applies to other settings urllib3 writes onto a caller-supplied context. It sets `context.verify_mode` and, when `assert_hostname`/`assert_fingerprint` are configured, `context.check_hostname = False`. An adapter that passes `assert_hostname=False` through `init_poolmanager` would therefore disable hostname checking on the shared context for all later `verify=True` requests in the process.

**Suggested direction (not applied):** when a client cert is given, do not use the shared context, or copy it first. Alternatively, apply the shared-context shortcut only when `client_cert is None`.

## Finding 2 (Medium-High): `verify=True` now overrides a user-supplied `ssl_context` set via `init_poolmanager`

**File:** `src/requests/adapters.py` lines 94-95.

**Evidence traced:**
- A common, documented customization is to subclass `HTTPAdapter` and call `init_poolmanager(..., ssl_context=ctx)` to set ciphers, a minimum TLS version, or similar.
- Per-request `pool_kwargs` override the `PoolManager` defaults in urllib3's `_merge_pool_kwargs`.
- Since `_urllib3_request_context` now always injects `ssl_context=_preloaded_ssl_context` for `verify=True`, the user's context is replaced.
- I confirmed this with a script, run against the head, using an adapter that passes a custom context:
  - `verify=True`: the pool's `ssl_context` is the module-global `_preloaded_ssl_context`, not the user's context (output `custom ctx used: False shared: True`).
  - `verify="/path/bundle"`: the user's context is still used (`True`).

**Failure:** a custom cipher list, TLS minimum or maximum version, or client-side policy configured by users is silently ignored under the default `verify=True`. This weakens or changes the TLS policy the user explicitly configured. It works only when `verify` is a path, so the behavior is inconsistent. The Session-level default is `verify=True`, so this is the common path.

## Finding 3 (Medium): `cert_verify()` no longer sets any CA source when `verify=True`

**File:** `src/requests/adapters.py` lines 297-318.

**Evidence traced:**
- `cert_verify` is a public, documented subclass hook, and it is called after the pool is chosen. Previously it always set `conn.ca_certs` to certifi's bundle for `verify=True`.
- Now, for `verify=True`, it only sets `cert_reqs`. The CA trust relies entirely on `_urllib3_request_context` having put the preloaded context on the pool.
- A subclass or third-party adapter that picks its connection through the deprecated `get_connection()` and then calls `cert_verify(conn, url, True, cert)` gets a pool with no certifi bundle. Examples include adapters that override `send`, or `get_connection_with_tls_context`, or `_get_connection` and build pools themselves. urllib3 then falls back to `context.load_default_certs()`, the OS trust store, instead of certifi.
- The CA source silently changes, and certifi-only environments may fail verification.
- The `OSError("Could not find a suitable TLS CA certificate bundle")` check for a missing default bundle is also gone for `verify=True`.

## Finding 4 (Low-Medium): import-time work and import-time failure

**File:** `src/requests/adapters.py` lines 75-78.

**Evidence traced:**
- `create_urllib3_context()`, `extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)` and `load_verify_locations` now run at `import requests`.
- This adds the slow certificate-loading cost (the one the PR measured at about 0.7s for 30 loads) to every import. That applies even to programs that never make an HTTPS request, or only use `verify=False` or a custom bundle.
- If the certifi bundle is missing or unreadable, or the zip extraction fails, `import requests` itself raises `OSError`/`FileNotFoundError`. Previously this surfaced only on the first verified request.
- Environments that strip or relocate certifi (some distro packages, frozen apps) now cannot import the library at all.
- The context also fixes the bundle at import time. Later changes to certifi data, or monkeypatching `DEFAULT_CA_BUNDLE_PATH`, are no longer honored.
