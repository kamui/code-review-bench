# Thermo Nuclear Review — psf/requests#6667 "Avoid reloading root certificates to improve concurrent performance"

Range reviewed: `8dd3b26bf59808de24fd654699f592abf6de581e..4089f3dc65f783beaa53cc032958ab625440d0ac` (`git diff main...review-head`), one file: `src/requests/adapters.py` (+28/−18).

What the change does: it creates a single module-global `ssl.SSLContext` at import time (`_preloaded_ssl_context`, lines 75–78) with the certifi bundle already loaded, hands that one object to urllib3 as the pool's `ssl_context` for every `verify=True` request (lines 94–95), and stops setting `conn.ca_certs` / `conn.ca_cert_dir` in `cert_verify()` when `verify is True` (lines 300–317). The performance goal is real, but a process-wide, mutable `SSLContext` shared by every pool, every `Session`, and every library in the interpreter is a different trust model from the per-pool contexts urllib3 created before, and urllib3 mutates the context it is handed. The findings below were all traced end to end against the installed urllib3 2.8.0 source and reproduced offline with local fixture servers; each reproduction was run against both the head and the merge-base sources so the regression is attributable to this diff.

Environment notes: Python 3.13.15, OpenSSL 3.5.8, urllib3 2.8.0, no network. The focused selection `tests/test_requests.py tests/test_adapters.py -k "tls or ssl or cert or verify or proxy or pyopenssl"` gives 32 passed / 3 failed at the head. Two failures (`test_pyopenssl_redirect`, `test_different_connection_pool_for_tls_settings_verify_bundle_unexpired_cert`) are the known pre-existing fixture/OpenSSL mismatches. The third, `test_different_connection_pool_for_mtls_settings`, fails on its very first request, which is `verify=False` (a path this diff does not touch), with `SSLV3_ALERT_CERTIFICATE_EXPIRED`; the fixture `tests/certs/mtls/client/client.pem` has `notAfter=Mar 13 18:35:04 2026 GMT`, so that is an expired test certificate, not a regression from this change.

---

## Finding 1: A client certificate given to one request is presented to every later `verify=True` connection in the process (shared mutable SSLContext)

**Priority:** High (security)

**Where:** `src/requests/adapters.py` lines 75–78 (module-global context), 94–95 (handed to every `verify=True` pool), 102–109 (`cert_file`/`key_file` go into the same pool kwargs).

**Evidence traced:**
- For `verify=True`, `_urllib3_request_context()` puts `_preloaded_ssl_context` into `pool_kwargs["ssl_context"]`; when `cert=` is also given it puts `cert_file`/`key_file` in the same `pool_kwargs`. `HTTPSConnectionPool._new_conn()` (urllib3 `connectionpool.py:1099–1115`) forwards `cert_file`, `key_file` and (via `conn_kw`) `ssl_context` to `HTTPSConnection`.
- `HTTPSConnection.connect()` → `_ssl_wrap_socket_and_match_hostname()` (urllib3 `connection.py:1031–1040`) uses the supplied context as-is, then `ssl_wrap_socket()` (urllib3 `util/ssl_.py:429–433`) executes `context.load_cert_chain(certfile, keyfile)` **on that shared object**. There is no copy. `SSLContext.load_cert_chain` replaces the context's client identity, and every socket subsequently wrapped from the context inherits it.
- Nothing ever unloads it. The context is a module global, so it is shared across all `HTTPAdapter` instances, all `Session`s, `requests.get()` one-shots, and every other library in the interpreter that uses requests.
- Reproduction (`clone-work/scratch/mtls_leak_demo.py`, two local trustme-backed TLS servers with `CERT_OPTIONAL`, no network): request 1 is `requests.get(url1, cert=(client.pem, client.key))`; request 2 is `requests.Session().get(url2)` to a different origin with **no** `cert`. At the head, server 2 receives the client certificate (`SAN=(('DNS', 'client-identity.example'),)`). At the merge-base, server 2 receives no client certificate.
- The PR thread already contains the warning that applies here (tiran: an `SSLContext` is safe to share "as long as you don't reconfigure it once it is used by a connection… changing … mTLS certs can lead to surprising behavior"). `load_cert_chain` on every mTLS connection is exactly that reconfiguration.

**Failure / vulnerability:**
- Credential/identity leak: any process that makes one mTLS call with `verify=True` will afterwards complete a client-certificate handshake with any server that asks for one, including servers chosen by untrusted input (webhook URLs, link fetchers, SSRF-adjacent code). The remote party learns the certificate (often containing identifying subject/SAN data) and obtains a proof-of-possession handshake from the application's private key.
- Wrong identity: applications that use different client certificates for different hosts/tenants now race on one context. `load_cert_chain` from thread A can land between thread B's `load_cert_chain` and B's `wrap_socket`, so B authenticates to its server with A's certificate. Which identity is presented depends on scheduling.
- Unexpected rejections: servers that request a certificate optionally and reject unknown issuers (common for corporate TLS front doors) will now fail connections that never asked for a client cert.
- Note `verify=<path>` is unaffected only because it does not use the shared context; the default and most common configuration (`verify=True`) is the one that leaks.

---

## Finding 2: Custom `ssl_context` / TLS settings configured by `HTTPAdapter` subclasses are silently replaced for every `verify=True` request

**Priority:** High (breaks existing functionality on a documented extension point)

**Where:** `src/requests/adapters.py` lines 94–95.

**Evidence traced:**
- Subclassing `HTTPAdapter.init_poolmanager()` (and `proxy_manager_for()`) to pass `ssl_context=`, `ssl_version=`, `ssl_minimum_version=`, etc. to `PoolManager` is the documented way to customise TLS (docs/user/advanced.rst, "Example: Specific SSL Version", lines 1008–1033, does exactly this with `ssl_version`). Those kwargs live in `PoolManager.connection_pool_kw`.
- `_get_connection()` calls `connection_from_host(..., pool_kwargs=pool_kwargs)`; urllib3 `PoolManager._merge_pool_kwargs()` (`poolmanager.py:390–410`) applies the request-level `pool_kwargs` **over** `connection_pool_kw`. With this diff, every `verify=True` request carries `ssl_context=_preloaded_ssl_context`, so the adapter's context is overwritten. For `verify=False` and `verify=<path>` no `ssl_context` is injected, so the custom context survives, producing inconsistent behaviour across the three values of `verify`.
- When urllib3 is given an `ssl_context`, it ignores `ssl_version`, `ssl_minimum_version` and `ssl_maximum_version` entirely (`connection.py:1031–1040` only consults them when `ssl_context is None`), so adapters that configure TLS versions rather than a context are broken the same way.
- Reproduction (`clone-work/scratch/static_checks.py`, no network): an adapter whose `init_poolmanager` passes a custom `ssl.SSLContext`. Head: `verify=True` → pool `ssl_context` is the module default, not the custom one; `verify=False` and `verify='/etc/hostname'` → custom one retained. Merge-base: custom context retained for all three. An adapter passing `ssl_minimum_version=TLSv1_3`: at the head the pool carries both that setting and the module default context, and urllib3 will use the context and ignore the minimum version.

**Failure / break:**
- Every existing adapter that supplies its own context (enterprise/corporate trust stores, `truststore.SSLContext`, lowered `@SECLEVEL`, legacy cipher suites, pinned TLS versions, custom `verify_flags`, unsafe-legacy-renegotiation for old appliances, client certs loaded into the context) stops working for the default `verify=True`, with no warning. Depending on what the custom context did, the symptom is either `SSLError` (custom CAs vanished, legacy server now refused) or a silent security downgrade (a context that pinned TLS 1.3 / a cipher list is replaced by the default one, so the restriction is no longer enforced).
- Because `verify=False` still honours the custom context, users debugging the breakage will see it "work" when they disable verification, which pushes exactly the wrong fix.

---

## Finding 3: `http://` URLs sent through an `https://` proxy with default `verify=True` now fail with a bare `ValueError`

**Priority:** High (hard failure of a supported configuration, escapes requests' exception hierarchy)

**Where:** `src/requests/adapters.py` lines 94–95 (context injected regardless of URL scheme) together with lines 318–321 (`cert_verify` sets `conn.cert_reqs = "CERT_NONE"` for non-https URLs).

**Evidence traced:**
- `_urllib3_request_context()` adds `ssl_context=_preloaded_ssl_context` for `verify=True` even when the request URL is `http://`. For an `http://` destination, urllib3's `ProxyManager.connection_from_host()` (`poolmanager.py:640–642`) returns a pool keyed on the **proxy** (scheme `https`), i.e. an `HTTPSConnectionPool` that now carries the shared context.
- `send()` then calls `cert_verify(conn, "http://…", True, None)`, which takes the `else` branch and sets `conn.cert_reqs = "CERT_NONE"` on that pool (pre-existing logic, but previously harmless because the pool had no context).
- On connect, the forwarding path `HTTPSConnection.connect()` → `_connect_tls_proxy()` (`connection.py:874–875`, `962–971`) uses `ssl_context = self.ssl_context` (the shared one, `check_hostname=True`) and `cert_reqs = CERT_NONE`, and `_ssl_wrap_socket_and_match_hostname()` executes `context.verify_mode = CERT_NONE` (`connection.py:1042`). The standard library refuses: `ValueError: Cannot set verify_mode to CERT_NONE when check_hostname is enabled.` `ValueError` is not an `OSError`/urllib3 error, so neither urllib3's retry loop nor `HTTPAdapter.send()`'s `except` clauses catch it.
- Reproduction (`clone-work/scratch/https_proxy_http_url_demo.py`, local TCP listener standing in for the proxy, no network): `requests.get("http://example.invalid/", proxies={"http": "https://127.0.0.1:<port>"})`. Head: `builtins.ValueError: Cannot set verify_mode to CERT_NONE when check_hostname is enabled.` Merge-base: `requests.exceptions.ProxyError` (it got as far as the TLS handshake with the fake proxy, i.e. the code path worked).
- Static confirmation in `static_checks.py`: at the head the pool for this case is an `HTTPSConnectionPool` with `cert_reqs=CERT_NONE` and the module-default `ssl_context`.

**Failure / break:** Every plain-HTTP request routed through a TLS-protected proxy (`proxies={"http": "https://…"}`, or `HTTP_PROXY=https://…` via `trust_env`) with the default `verify=True` raises an uncaught `ValueError` before any bytes are sent. Callers that catch `requests.exceptions.RequestException` do not catch it. Setting `verify=False` "fixes" it because that path does not inject the context, again steering users toward disabling verification.

---

## Finding 4: `get_connection()` + `cert_verify(verify=True)` no longer loads any CA bundle; verification silently switches from certifi to the OS default store

**Priority:** Medium (behaviour change on a public, documented hook; hard failure on hosts without an OS trust store)

**Where:** `src/requests/adapters.py` lines 300–317 (the removed `conn.ca_certs = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)` fallback) and the new comment on lines 300–303.

**Evidence traced:**
- `HTTPAdapter.get_connection()` (lines 406–433) is public, documented "for use when subclassing", and not deprecated at this head; it builds pools with `connection_from_url(url)` and therefore never receives `_preloaded_ssl_context`. Third-party adapters that override `send()` call `self.get_connection(...)` followed by `self.cert_verify(...)`, mirroring what `HTTPAdapter.send()` itself did until the `_get_connection` refactor on `main`.
- Before this diff, `cert_verify()` guaranteed certifi on that path by setting `conn.ca_certs` whenever `verify` was truthy. After this diff, for `verify is True` it sets only `conn.cert_reqs` and returns. The new comment ("the connection will be using a context with the default certificates already loaded") is only true for pools obtained through `_get_connection()`.
- With `ca_certs=None`, `ca_cert_dir=None`, `ssl_context=None`, urllib3 builds a fresh context and calls `context.load_default_certs()` (`connection.py:1061–1068`), i.e. the operating-system store.
- Reproduction (`static_checks.py`, case E): head → `cert_reqs=CERT_REQUIRED ca_certs=None ca_cert_dir=None ssl_context=None`; merge-base → `ca_certs=<certifi cacert.pem>`.

**Failure / break:** For any adapter still on the public `get_connection()` path, `verify=True` now trusts whatever the OS provides instead of certifi. On python.org macOS builds without "Install Certificates.command", in minimal/distroless containers, or in zipapp/PyInstaller bundles where the only CA bundle is the one shipped with certifi, every verified HTTPS request through such an adapter fails with `CERTIFICATE_VERIFY_FAILED`. Where the OS store differs from certifi, which CAs are trusted changes silently.

---

## Finding 5: HTTPS proxies (`https://` proxy, HTTPS destination) are now authenticated against the OS default store instead of certifi

**Priority:** Medium

**Where:** `src/requests/adapters.py` lines 300–317 (no `ca_certs` set for `verify=True`); lines 94–95 only cover the tunnelled destination, not the proxy hop.

**Evidence traced:**
- For an `https://` destination via an `https://` proxy, the destination pool receives `ssl_context=_preloaded_ssl_context` but, after this diff, `pool.ca_certs` stays `None` (before: certifi path set by `cert_verify`). `static_checks.py` case D: head → `pool.ca_certs=None`, `proxy_config.ssl_context=None`; merge-base → `pool.ca_certs=<certifi cacert.pem>`.
- The CONNECT tunnel's TLS to the proxy is made by `_connect_tls_proxy()` (`connection.py:943–993`). With no `proxy_ssl_context` and a tunnelling (non-forwarding) connection it uses `ssl_context=None` and `ca_certs=self.ca_certs`, so `_ssl_wrap_socket_and_match_hostname()` creates a default context and, with `ca_certs` now `None`, falls back to `load_default_certs()` (`connection.py:1061–1068`). The shared certifi-loaded context is used only for the inner TLS-in-TLS leg.

**Failure / break:** The proxy's certificate is no longer checked against the bundle requests documents as its trust root. On systems without a populated OS store the handshake to the proxy fails (`ProxyError` wrapping `CERTIFICATE_VERIFY_FAILED`); elsewhere the trusted-CA set for the proxy hop silently changes. Users who relied on `verify=True` meaning "certifi" for the proxy now get different results from the destination leg.

---

## Finding 6: `import requests` now performs TLS setup eagerly and fails outright where the old code only failed verified HTTPS requests

**Priority:** Medium (devex / robustness regression)

**Where:** `src/requests/adapters.py` lines 75–78.

**Evidence traced:**
- `create_urllib3_context()` raises `TypeError("Can't create an SSLContext object without an ssl module")` when the interpreter has no `ssl` (urllib3 `util/ssl_.py:223–224`). urllib3 itself and requests' `__init__` tolerate a missing `ssl` module; this module-level call does not. Reproduced: with `ssl` unavailable, head → `TypeError` at `import requests`; merge-base → import succeeds (plain-HTTP use was possible).
- `load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))` runs at import. If the bundle path does not exist or is unreadable (distro-patched `certs.where()` pointing at an absent system bundle, broken/empty certifi install, read-only or stripped deployments), the import raises a raw `FileNotFoundError`/`ssl.SSLError`. Reproduced: with `certifi.where()` returning a missing path, head → `FileNotFoundError` at `import requests`; merge-base → import OK, and only a verified HTTPS request would later raise requests' descriptive `OSError("Could not find a suitable TLS CA certificate bundle, invalid path: …")`.
- `extract_zipped_paths()` (utils.py:268–302) writes the bundle into `tempfile.gettempdir()` when requests runs from a zip/PyInstaller archive. That filesystem write now happens at import time in every process, instead of lazily on the first verified HTTPS request.

**Failure / break:** Programs that only ever use `http://`, `verify=False`, or `verify=<custom path>` now fail to import requests in the environments above, and the error surfaces as an import-time stack trace in urllib3/ssl rather than requests' own message. Import of requests also becomes measurably heavier for every process (the slow `load_verify_locations` the PR is trying to avoid per-request is now paid unconditionally at import, including by processes that never make a verified HTTPS request).

---

## Checked and not reported

- Thread-safety of merely sharing an `SSLContext` across connections is fine per OpenSSL/CPython guidance; the problems above come from urllib3 *reconfiguring* the shared context (`load_cert_chain`, `verify_mode`, `check_hostname`), not from concurrent `wrap_socket`.
- `_ssl_wrap_socket_and_match_hostname()` also flips `context.check_hostname = False` on the shared context under pyOpenSSL injection or LibreSSL builds; urllib3 then matches the hostname itself (`connection.py:1098–1119`), so I found no hostname-verification bypass from that mutation.
- The `verify=None` path (pool keyed `CERT_REQUIRED`, then `cert_verify` downgrades to `CERT_NONE`) is unchanged by this diff and is pre-existing behaviour.
- The `os.path.isdir()` split added to `_urllib3_request_context()` for string `verify` values is correct and consistent with `cert_verify()`.
- The import-time `extract_zipped_paths()` call preserves zip/PyInstaller support that an earlier revision of the PR had dropped; the reviewer's original objection on that point is addressed at this head.
