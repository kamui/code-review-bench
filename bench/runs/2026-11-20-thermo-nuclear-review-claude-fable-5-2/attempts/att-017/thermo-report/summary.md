# Thermo-nuclear review — psf/requests#6667

"Avoid reloading root certificates to improve concurrent performance"
Range `8dd3b26b..4089f3dc` (`main...review-head`), one file: `src/requests/adapters.py` (+28 / −18).

## Verdict

Nine actionable findings: three High, four Medium, two Low. Do not merge as written.

The change replaces "urllib3 builds a fresh `SSLContext` per connection and loads certifi into it" with "every `verify=True` connection in the process shares one module-global `SSLContext`". urllib3 does not treat a supplied `ssl_context` as read-only: on every connection it writes `verify_mode`, may write `check_hostname`, loads any `ca_certs`/`ca_cert_dir`/`ca_cert_data` into it and loads the client certificate into it. Per-request and per-adapter TLS settings therefore now land in process-wide state, and adapter-level TLS settings that used to apply are overridden. Separately, removing `conn.ca_certs` from `cert_verify()` changes the trust store on every path that does not carry the shared context.

## How this was checked

- Read the diff, the full head `adapters.py`, and the urllib3 code it drives (the installed urllib3 2.8.0: `connection.py`, `poolmanager.py`, `util/ssl_.py`).
- Wrote scratch probes outside the clone (`clone-work/scratch/test_probe.py`, `test_probe2.py`, `test_probe_import.py`) and ran them with pytest against the head, using local TLS servers with trustme certificates and local TCP listeners. Environment: Python 3.13.15, OpenSSL 3.5.8, urllib3 2.8.0. All 13 probes passed, i.e. every behaviour described as "confirmed" below was observed at the head.
- Each probe swaps `_preloaded_ssl_context` for a fresh context built exactly as `adapters.py:75-78` builds it, so probes do not contaminate each other. Where a probe needs a server that `verify=True` accepts, it adds the test CA to that context as setup; findings 2, 5 and 8 use no such setup.
- Base behaviour was established by reading `main`, not by running it. Statements about urllib3 1.26.x are from knowledge of that code and were not executed here; they are marked where they occur.
- Repo tests: `tests/test_adapters.py` plus the TLS/verify/cert/pool/proxy selection of `tests/test_requests.py` gave 38 passed, 3 failed. Two failures are the known fixture mismatch named in the task (`test_pyopenssl_redirect`, `..._verify_bundle_unexpired_cert`). The third, `test_different_connection_pool_for_mtls_settings`, fails on its first request, which uses `verify=False` and so never touches the changed path; the server rejects the fixture client certificate `tests/certs/mtls/client/client.pem`, whose `notAfter` is 13 Mar 2026 (today is 2026-11-20). None of the three is caused by this change.
- No existing test covers any of the behaviours below. The PR adds no tests; the author says so in the review thread.

## Findings

### 1. [High] A client certificate passed with `cert=` is loaded into the process-global context and is then presented by every other session to every other host

**Where:** `src/requests/adapters.py:94-95` (with `75-78` and the unchanged client-cert handling at `102-109`).

**Evidence.** With `verify=True`, `_urllib3_request_context` puts the shared `_preloaded_ssl_context` into `pool_kwargs` and, when `cert` is given, also `cert_file`/`key_file`. urllib3 passes all three to `ssl_wrap_socket`, which does `context.load_cert_chain(certfile, keyfile)` on the supplied context (`urllib3/util/ssl_.py:429-433`). At `main` that context was created per connection, so the certificate lived only on that connection. At the head it is the module global, so the certificate and key stay installed on the context used by every later `verify=True` connection in the process, from any `Session`, to any host. Pools are keyed by `cert_file`, but the context is shared across pools, so the pool key gives no isolation.

Confirmed at the head (`test_p1_client_cert_leaks_to_other_sessions_and_hosts`): a certless request to server B presents no client certificate; after one `Session().get(A, cert=client.pem)`, a brand-new `Session().get(B)` with no `cert=` presents that client certificate to B (the server's `getpeercert()` returns the `client@example.com` certificate).

**Failure.**
- Credential and identity disclosure: after any request that uses `cert=`, the process authenticates with that mTLS identity to any other HTTPS server that asks for a client certificate, including requests made by unrelated libraries through `requests.get()`. A request the caller meant to be anonymous is authenticated.
- Breakage: a server that optionally requests client certificates from a different CA now rejects the handshake. Confirmed (`test_p1b_...`): the same certless request that returned 200 before fails with `SSLError: TLSV1_ALERT_UNKNOWN_CA` after an unrelated `cert=` request elsewhere in the process.
- Wrong identity under concurrency: two threads using different client certificates both call `load_cert_chain` on the same context before creating their sockets, so a connection can be made with the other thread's certificate. This follows from the confirmed mechanism; the race itself was not reproduced.

The reviewer comment quoted in the PR discussion ("changing ciphers, verification settings, or mTLS certs can lead to surprising behavior") describes exactly this, and the shared context is reconfigured on that path.

### 2. [High] An `ssl_context` supplied by an adapter through `init_poolmanager` / `proxy_manager_for` is silently replaced by the global context

**Where:** `src/requests/adapters.py:94-95`.

**Evidence.** `pool_kwargs["ssl_context"] = _preloaded_ssl_context` is passed as a per-request override to `connection_from_host(..., pool_kwargs=pool_kwargs)` (`adapters.py:395-401`). urllib3's `_merge_pool_kwargs` lets override keys win over the `PoolManager`'s own `connection_pool_kw` (`urllib3/poolmanager.py:400-409`). The supported way to customise TLS in requests is a subclass that passes `ssl_context=` to `init_poolmanager` (or `proxy_manager_for`); that value sits in `connection_pool_kw` and is now always overridden when `verify` is `True`, which is the default. At `main` no `ssl_context` override was sent, so the adapter's context was used.

Confirmed at the head (`test_p2_custom_ssl_context_from_init_poolmanager_is_ignored`): an adapter whose context trusts a private CA is mounted; the context actually handed to urllib3's TLS wrap is `_preloaded_ssl_context`, not the adapter's, and the request fails with `CERTIFICATE_VERIFY_FAILED`.

**Failure.** Everything configured on such a context is dropped without warning for default-verify requests: private or pinned trust stores (requests fail, or, where the context was meant to restrict trust, the full certifi set is trusted instead), client certificates loaded with `load_cert_chain` (mTLS stops working), cipher lists, minimum TLS versions and option flags (security policy silently reverts to urllib3 defaults), and OS trust-store contexts. The same adapter still works with `verify=False` or `verify=<path>`, which makes the failure confusing to diagnose.

### 3. [Medium] `ssl_version`, `ssl_minimum_version` and `ssl_maximum_version` passed through `init_poolmanager` are silently ignored for `verify=True`

**Where:** `src/requests/adapters.py:94-95`.

**Evidence.** urllib3 applies these keyword arguments only when it builds the context itself: `if ssl_context is None: context = create_urllib3_context(ssl_version=..., ssl_minimum_version=..., ssl_maximum_version=...) else: context = ssl_context` (`urllib3/connection.py:1030-1040`). Because the head always supplies a context for `verify=True`, the adapter's version settings are never read. This is the pattern the project's own documentation teaches (`docs/user/advanced.rst:1008-1033`, "Example: Specific SSL Version", `PoolManager(..., ssl_version=...)`). It is a separate path from finding 2: a fix that only respects an adapter-supplied `ssl_context` leaves this broken.

Confirmed at the head (`test_p3_ssl_minimum_version_pool_kwarg_ignored`): an adapter with `ssl_minimum_version=TLSv1_3` completes a default-verify request against a TLS 1.2-only server (server records `TLSv1.2`, response 200). The same adapter with `verify=<ca path>` correctly fails with `TLSV1_ALERT_PROTOCOL_VERSION`.

**Failure.** A TLS version floor enforced through an adapter is silently removed, a downgrade of security policy with no error. Adapters that pin or lower the version to reach a legacy endpoint stop connecting. Behaviour now differs between `verify=True` and `verify=<path>` for the same adapter.

### 4. [Medium] `http://` requests through an `https://` proxy now raise a bare `ValueError`; under some conditions they switch the global context to `CERT_NONE` instead

**Where:** `src/requests/adapters.py:94-95`, interacting with the unchanged `cert_verify` branch at `318-321`.

**Evidence.** `ssl_context` is added for `verify is True` regardless of the request scheme. For a direct `http://` request urllib3 strips it, but for an `http://` request routed through an `https://` proxy, `ProxyManager.connection_from_host` builds an HTTPS pool to the proxy with the same `pool_kwargs` (`urllib3/poolmanager.py:635-642`), so the pool carries the shared context. `cert_verify` then sees a URL that does not start with `https` and sets `conn.cert_reqs = "CERT_NONE"` on that pool. On connect urllib3 executes `context.verify_mode = resolve_cert_reqs(cert_reqs)` on the supplied context (`urllib3/connection.py:1042`, reached through `_connect_tls_proxy`, `connection.py:962-965`). At `main` the same `CERT_NONE` was applied to a throwaway per-connection context and the request went through.

Confirmed at the head:
- `test_p4_http_via_https_proxy_default_verify`: `Session().get("http://…", proxies={"http": "https://127.0.0.1:<port>"})` raises `ValueError: Cannot set verify_mode to CERT_NONE when check_hostname is enabled.` It is not a `requests` exception, so `except RequestException` does not catch it. `HTTP_PROXY=https://…` in the environment triggers the same path.
- `test_p4b_global_context_downgraded_to_cert_none`: once any adapter using `assert_hostname=False` (or `assert_fingerprint`) has made a default-verify connection, urllib3 has set `check_hostname = False` on the global context (`connection.py:1045-1056`) and nothing sets it back. The same proxied request then no longer raises; it leaves `_preloaded_ssl_context.verify_mode == CERT_NONE` on the process-global context.

**Failure.** A previously working proxy configuration fails on every plain-HTTP request with an uncaught `ValueError`. In the second state the shared context is left with verification off until the next default-verify connection writes `CERT_REQUIRED` back, so a concurrent thread that creates its TLS socket in that window handshakes with an unverified peer; urllib3's own post-handshake hostname check is also skipped while `verify_mode` is `CERT_NONE` (`connection.py:1098-1102`). The window was not reproduced as a race, only the state that opens it. On urllib3 1.26.x, which `setup.py` still allows (`urllib3>=1.21.1,<3`), contexts from `create_urllib3_context` have `check_hostname` off from the start, so I expect the `CERT_NONE` write rather than the `ValueError` on every such request; that was not executed here. A subclass that overrides `cert_verify` to force `CERT_NONE` while `verify` stays `True` reaches the same line.

### 5. [Medium] CA material configured on one adapter is permanently added to the trust store used by every other session

**Where:** `src/requests/adapters.py:94-95` (with `75-78`).

**Evidence.** `ssl_wrap_socket` calls `context.load_verify_locations(ca_certs, ca_cert_dir, ca_cert_data)` on the supplied context whenever any of those is set on the connection (`urllib3/util/ssl_.py:413-415`). An adapter can set them at `PoolManager` level through `init_poolmanager(**pool_kwargs)` or `proxy_manager_for(**proxy_kwargs)`; `ca_cert_data` in particular has no `verify=` equivalent. At `main` they were loaded into a per-connection context. At the head they are loaded into `_preloaded_ssl_context`.

Confirmed at the head (`test_p5_ca_cert_data_pollutes_global_trust`), with no test setup touching the global context: a plain `Session().get()` to a server signed by a private CA fails; one request through an adapter with `ca_cert_data=<private CA>` succeeds; afterwards `_preloaded_ssl_context.cert_store_stats()["x509_ca"]` has gone from 121 to 122 and a plain, unrelated `Session().get()` to that server returns 200.

**Failure.** Trust granted to a private or interception CA for one adapter or one mount prefix silently becomes trust for every default-verify request in the process, for the life of the process, including requests from other libraries. Which hosts a process trusts now depends on which adapters happened to connect earlier.

### 6. [Medium] TLS to an `https://` proxy is now verified against the OS trust store instead of the certifi bundle

**Where:** `src/requests/adapters.py:297-317` (`cert_verify` no longer sets `conn.ca_certs` / `conn.ca_cert_dir` when `verify is True`).

**Evidence.** For an `https://` URL tunnelled through an `https://` proxy, urllib3 deliberately does not use the pool's `ssl_context` for the proxy leg: `ssl_context = self.ssl_context if self.proxy_is_forwarding else None`, while `ca_certs`, `ca_cert_dir` and `cert_reqs` are inherited from the pool (`urllib3/connection.py:962-971`). At `main` `cert_verify` had set `ca_certs` to the certifi bundle, so the proxy certificate was checked against certifi. At the head `ca_certs` is `None`, so urllib3 builds a default context and calls `load_default_certs()` (`connection.py:1061-1068`).

Confirmed at the head (`test_p6_https_proxy_tls_uses_os_store_not_certifi`): the TLS wrap for the proxy leg is called with `ssl_context=None, ca_certs=None, ca_cert_dir=None, ca_cert_data=None, cert_reqs='CERT_REQUIRED'`.

**Failure.** HTTPS proxies whose certificate chains to a certifi root stop verifying on hosts with an empty or absent OpenSSL default store (minimal containers without a CA package, Python builds whose default cert path is unpopulated), failing with `CERTIFICATE_VERIFY_FAILED`. In the other direction, the set of CAs trusted to authenticate the proxy silently changes from certifi to whatever the OS store holds. This contradicts `docs/user/advanced.rst:652-657`, which tells users the proxy's root must be in `DEFAULT_CA_BUNDLE_PATH`. The origin leg is unaffected; it uses the shared context.

### 7. [Medium] `import requests` now fails when the default CA bundle is missing, is a directory, or the interpreter has no `ssl` module

**Where:** `src/requests/adapters.py:75-78`.

**Evidence.** The context is created and the bundle loaded at module import, with no error handling. At `main` the bundle was only touched inside `cert_verify`, only for HTTPS with `verify=True`, and raised a descriptive `OSError("Could not find a suitable TLS CA certificate bundle, invalid path: …")`; a directory was supported by routing it to `ca_cert_dir`. `requests/__init__.py` imports `adapters` unconditionally via `sessions`.

Confirmed at the head (`test_p8_import_fails_when_default_bundle_unusable`, by reloading `requests.adapters` with `requests.utils.DEFAULT_CA_BUNDLE_PATH` pointed elsewhere): a missing path raises `FileNotFoundError: [Errno 2] No such file or directory` and a directory raises `IsADirectoryError: [Errno 21] Is a directory`, both during module execution. By reading: `create_urllib3_context()` raises `TypeError("Can't create an SSLContext object without an ssl module")` when `ssl` is unavailable (`urllib3/util/ssl_.py:223-224`), a configuration `requests/__init__.py:119-124` explicitly tolerates.

**Failure.** Environments where `certs.where()` points at a path that is absent or unreadable (repackaged distributions that redirect it to a system path, stripped images) can no longer import requests at all, including programs that only use `http://`, `verify=False`, or `REQUESTS_CA_BUNDLE`/`verify=<path>` and never needed the default bundle. The error no longer names the path. ssl-less interpreters lose plain-HTTP use of requests. In zip-packaged deployments `extract_zipped_paths` now writes the bundle to the temp directory as an import side effect, so an unwritable temp directory fails the import as well. The author's review reply acknowledges dropping the directory check on the assumption that the default is always a file.

### 8. [Low] Overriding `requests.adapters.DEFAULT_CA_BUNDLE_PATH` at runtime no longer has any effect

**Where:** `src/requests/adapters.py:75-78` and `297-317`.

**Evidence.** At `main` `cert_verify` read the module global `DEFAULT_CA_BUNDLE_PATH` on every request, so assigning to `requests.adapters.DEFAULT_CA_BUNDLE_PATH` after import changed the bundle used for `verify=True`. At the head the name is read once, at import, to fill the shared context, and `cert_verify` no longer refers to it.

Confirmed at the head (`test_p9_runtime_default_bundle_override_ignored`): with `adapters.DEFAULT_CA_BUNDLE_PATH` set to a private CA file, a default-verify request to a server signed by that CA fails with `CERTIFICATE_VERIFY_FAILED`.

**Failure.** Applications and repackagers that redirect the default bundle by patching this attribute silently fall back to certifi and start failing verification against private CAs. More generally, everything urllib3 reads when it builds a context (for example `SSLKEYLOGFILE`) is now sampled once at import for default-verify requests rather than per connection. `REQUESTS_CA_BUNDLE` / `CURL_CA_BUNDLE` still work because they turn `verify` into a path.

### 9. [Low] The public `get_connection()` + `cert_verify()` subclass path no longer configures any trust store for `verify=True`

**Where:** `src/requests/adapters.py:297-317`.

**Evidence.** `cert_verify` is documented as "exposed for use when subclassing", as is `get_connection` (`adapters.py:406`). At `main`, `cert_verify(conn, url, True, cert)` made any pool verify against certifi by setting `conn.ca_certs`. At the head it only sets `cert_reqs`; the trust store now arrives solely through the private `_urllib3_request_context` / `_get_connection` path. A pool obtained any other way has neither.

Confirmed at the head (`test_p7_get_connection_plus_cert_verify`): after `conn = adapter.get_connection(url); adapter.cert_verify(conn, url, True, None)` the pool has `cert_reqs='CERT_REQUIRED'`, `ca_certs=None`, `ca_cert_dir=None` and no `ssl_context`, so urllib3 falls back to `load_default_certs()`.

**Failure.** Third-party adapters that implement their own `send()` with these two hooks (the shape of `HTTPAdapter.send` before `_get_connection` existed) silently switch from certifi to the OS trust store, with the same consequences as finding 6. Rated Low because it only affects adapters that re-implement `send()`; the stock `send()` is not affected.

## Checked and not reported

- Pool separation: `verify=True`, `verify=False` and `verify=<path>` still map to different pools (`key_ssl_context`, `key_cert_reqs`, `key_ca_certs` / `key_ca_cert_dir`), and direct `http://` requests have the TLS keywords stripped by urllib3.
- The new `os.path.isdir(verify)` branch in `_urllib3_request_context` (`adapters.py:96-100`) is correct and matches `cert_verify`.
- Cost of building the context at import: measured at about 3 ms on this machine; not worth a finding.
- Sharing one `SSLContext` across threads purely for handshakes is safe, as the PR discussion concludes. The findings above are about the context being reconfigured after first use, which that discussion names as the unsafe case.
