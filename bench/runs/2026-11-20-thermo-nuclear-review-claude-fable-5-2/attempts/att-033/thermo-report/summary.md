# Thermo-nuclear review — psf/requests#6667

"Avoid reloading root certificates to improve concurrent performance", range `8dd3b26b..4089f3dc`, one file changed: `src/requests/adapters.py`.

## Verdict

The change should not ship as written. It has seven actionable findings: two high, three medium, two low.

The root cause is shared by most of them. The PR creates one `SSLContext` at import time and hands that same object to every `verify=True` pool in the process. The PR discussion concluded this is safe "as long as you don't reconfigure it once it is used by a connection". urllib3 reconfigures the context it is given on every new connection: it assigns `verify_mode`, may clear `check_hostname`, and calls `load_cert_chain` when a client certificate is set. Each of those writes now lands on process-global state.

The second cause is that `cert_verify()` no longer sets `ca_certs` for `verify=True`. Any TLS handshake that does not use the pool's `ssl_context` loses the certifi bundle.

## How this was checked

- I read the diff, the full head version of `adapters.py`, and the installed urllib3 2.8.0 sources that consume the changed values.
- I ran eleven scratch probes through pytest against the head, on Python 3.13.15 with OpenSSL 3.5.8, using local TLS listeners and `trustme` certificates. All eleven passed, meaning each asserted the behaviour described below and observed it. The probes are in `../scratch/test_pr6667_scratch.py` and `../scratch/test_pr6667_scratch2.py`.
- I ran the existing TLS, proxy, adapter and pool tests: 38 passed, 3 failed. Two are the known fixture failures named in the run policy. The third, `test_different_connection_pool_for_mtls_settings`, fails on its `verify=False` request because the client fixture certificate expired on 2026-03-13; that path is not changed by this PR.
- Base behaviour was established by reading the base code, not by running it.
- urllib3 1.26 is not installed, so statements about it are marked as unverified.
- One command fell outside the pytest-only allowance: a read-only Python one-liner that printed the expiry dates of three fixture certificates. It changed nothing in the clone, which was clean after every run.

## Findings

### 1. [High] A client certificate passed with `cert=` is loaded into the global context and presented by unrelated sessions

**Where:** `src/requests/adapters.py` lines 75–78 (the module-level context), 94–95 (it is injected for `verify=True`), 102–109 (`cert_file` / `key_file` go into the same pool).

**Evidence.** For `verify=True` with `cert=...`, the pool receives both `ssl_context=_preloaded_ssl_context` and `cert_file`/`key_file`. On connect, urllib3's `ssl_wrap_socket` runs `context.load_cert_chain(certfile, keyfile)` on the context it was given (`urllib3/util/ssl_.py` lines 429–433). That context is now the process-wide one, so the certificate and private key stay loaded in it for the life of the process. Nothing ever unloads them. At the base, no `ssl_context` was passed, so urllib3 built a fresh context per connection and the certificate went no further than that connection.

A probe confirmed it end to end against a local TLS server that requests (but does not require) a client certificate:

1. A new session with no `cert=`: the server saw no client certificate.
2. A session calling `get(url, cert=(client.pem, client.key))`: the server saw the client certificate, as expected.
3. A brand-new `Session`, a different host name, no `cert=` at all: the server saw the same client certificate.
4. Control with `verify="<ca path>"`, which does not use the shared context: no client certificate.

The certificate is loaded before the handshake, so it leaks even when the original request fails verification. The existing test `test_different_connection_pool_for_mtls_settings` does exactly this with `s.get(url, cert=cert)`.

**Failure.** After any one `verify=True` request with a client certificate, every later `verify=True` connection in the process presents that certificate to any server that asks for one. This crosses sessions, adapters, hosts and libraries. The server learns the certificate's identity and gets proof of possession, and requests the caller meant to send anonymously are authenticated as that identity where the certificate is accepted. In a process that uses different client certificates on different threads, `load_cert_chain` and the socket wrap are not atomic on the shared context, so one thread's connection can go out with another thread's certificate; that interleaving was not reproduced here, only the sequential leak was.

### 2. [High] Adapters that supply their own `ssl_context` or TLS pool options have them silently replaced when `verify=True`

**Where:** `src/requests/adapters.py` lines 94–95.

**Evidence.** `pool_kwargs["ssl_context"] = _preloaded_ssl_context` is passed as a per-request override to `connection_from_host`. urllib3's `_merge_pool_kwargs` lets request overrides win over the `PoolManager`'s own keyword arguments, so an `ssl_context` given in `init_poolmanager` is discarded. At the base the override contained only `cert_reqs`, `ca_certs`, `cert_file` and `key_file`, so the adapter's context was kept.

Version options are affected too. urllib3 reads `ssl_version`, `ssl_minimum_version` and `ssl_maximum_version` only when it builds a context itself (`urllib3/connection.py` lines 1031–1038), so they become dead once a context is injected.

Probes confirmed:

- With an adapter whose `init_poolmanager` passes `ssl_context=ctx`, the pool for `verify=True` holds `_preloaded_ssl_context`, while the pools for `verify=False` and `verify="<path>"` still hold `ctx`.
- End to end, an adapter whose context trusts a private CA failed against a server signed by that CA with `CERTIFICATE_VERIFY_FAILED: unable to get local issuer certificate` on a default `session.get()`.
- An adapter passing `ssl_minimum_version=TLSv1_3` got a pool whose context still has minimum TLS 1.2.

**Failure.** `verify=True` is the default, so every customisation made through the documented transport-adapter route stops applying on the default path, with no error: minimum TLS version, cipher lists, OpenSSL option flags, private or system trust stores loaded into the context, and client certificates loaded into the context (for example with a key password). Adapters that tighten TLS are silently downgraded to urllib3 defaults, and adapters that add trust or client identity start failing. The repository's own documented example, the "Specific SSL Version" adapter at `docs/user/advanced.rst` lines 1008–1033, becomes a no-op for `verify=True`. Behaviour now also differs by `verify` value on the same adapter, because `verify=False` and a bundle path still use the adapter's context.

Not verified here: on urllib3 1.26 the context is built at import, so process-wide tweaks made after `import requests` (such as editing `urllib3.util.ssl_.DEFAULT_CIPHERS`) would likewise stop applying.

### 3. [Medium] An `http://` URL sent through an `https://` proxy now raises `ValueError` with default settings

**Where:** `src/requests/adapters.py` lines 94–95, combined with the unchanged `else` branch of `cert_verify` at lines 318–321.

**Evidence.** For an `http://` request with `verify=True`, `_urllib3_request_context` still injects the shared context. With an `https://` proxy, urllib3's `ProxyManager.connection_from_host` returns an HTTPS pool to the proxy and keeps that `ssl_context`. `cert_verify` then sees a URL that does not start with `https` and sets `conn.cert_reqs = "CERT_NONE"`. On connect, urllib3 runs `context.verify_mode = resolve_cert_reqs(cert_reqs)` on the supplied context (`urllib3/connection.py` line 1042). The shared context has `check_hostname=True`, and Python refuses that combination.

A probe confirmed it: `session.get("http://example.invalid/", proxies={"http": "https://localhost:<port>"})` raised `ValueError: Cannot set verify_mode to CERT_NONE when check_hostname is enabled.` The same request with `verify=False` returned 200. At the base no context was passed, urllib3 built a `CERT_NONE` context itself, and the request went through.

**Failure.** Every plain-HTTP request routed through an HTTPS proxy fails, including when the proxy comes from `HTTP_PROXY=https://...`. The exception is a bare `ValueError`, not a `RequestException`, so existing `except requests.RequestException` handlers do not catch it. This is deterministic on urllib3 2.x with the standard `ssl` module; the affected population is limited to users of HTTPS proxies.

### 4. [Medium] The TLS handshake with an `https://` proxy is no longer verified against the certifi bundle

**Where:** `src/requests/adapters.py` lines 297–317 (`cert_verify` no longer sets `conn.ca_certs` or `conn.ca_cert_dir` when `verify is True`).

**Evidence.** When an `https://` URL is tunnelled through an `https://` proxy, urllib3 verifies the proxy in `_connect_tls_proxy`. Without a `proxy_ssl_context` (requests never passes one) it deliberately does not use the pool's `ssl_context` for a tunnelling proxy; it builds a new context from the connection's `cert_reqs`, `ca_certs` and `ca_cert_dir` (`urllib3/connection.py` lines 962–971). At the base `ca_certs` was the certifi path. At the head it is `None`, so urllib3 falls back to `context.load_default_certs()`, the operating system store (lines 1061–1068).

A probe confirmed it: after `cert_verify(pool, url, True, None)` the pool's `ca_certs` and `ca_cert_dir` are both `None`, and during a request through a local HTTPS proxy the only trust-store call was `load_default_certs()`; `load_verify_locations` was never called.

**Failure.** With default settings, the proxy's certificate is checked against the OS trust store, not the certifi bundle. On systems with no usable OS store (minimal containers without CA certificates, Python installs whose OpenSSL default paths are empty) requests through an HTTPS proxy start failing with `ProxyError ... CERTIFICATE_VERIFY_FAILED`. Elsewhere the set of authorities trusted for the proxy hop silently changes. It also contradicts `docs/user/advanced.rst` lines 652–657, which tell users the proxy's root must be in `DEFAULT_CA_BUNDLE_PATH`. Setting `REQUESTS_CA_BUNDLE` or passing a path still works, because string values are still copied to `ca_certs`.

### 5. [Medium] `import requests` now fails outright where only verified HTTPS requests used to fail

**Where:** `src/requests/adapters.py` lines 75–78.

**Evidence.** The context is created and the bundle is loaded while `requests.adapters` is being imported, and `requests/__init__.py` imports that module. Three cases break:

- **Missing bundle file.** A probe that pointed `DEFAULT_CA_BUNDLE_PATH` at a nonexistent file and re-imported the module got `FileNotFoundError: [Errno 2] No such file or directory`. At the base the same condition raised `OSError("Could not find a suitable TLS CA certificate bundle, invalid path: ...")`, and only from `cert_verify` on an HTTPS request with `verify=True`.
- **Bundle path that is a directory.** The base handled this with `os.path.isdir` and `ca_cert_dir`. The new code passes the path as `cafile`; a probe got `IsADirectoryError`. `requests/certs.py` explicitly invites repackagers to redefine `where()`, so a non-certifi path is a supported setup. The author noted in the review thread that the `isdir` check was skipped.
- **Python built without the `ssl` module.** `urllib3.util.ssl_` sets `SSLContext = None` and `create_urllib3_context()` raises `TypeError("Can't create an SSLContext object without an ssl module")` (`urllib3/util/ssl_.py` lines 16 and 223–224). `requests/__init__.py` lines 117–124 still explicitly cater for a missing `ssl` module. This case was traced by reading, not run.

**Failure.** In these environments the whole package becomes unimportable. Programs that only make plain-HTTP requests, use `verify=False`, or set `REQUESTS_CA_BUNDLE` worked before and now crash at import, with a low-level error that no longer names the bundle path. Frozen applications that omit the certifi data file and distribution-patched bundle locations are the likely victims.

### 6. [Low] Per-adapter settings leak into the global context; one of them opens a window in which verification is switched off

**Where:** `src/requests/adapters.py` lines 75–78 and 94–95, with the `CERT_NONE` assignment at line 319.

**Evidence.** urllib3 writes to the context it is handed on every connect: `context.verify_mode = ...` and, when `assert_hostname` or `assert_fingerprint` is set, `context.check_hostname = False` (`urllib3/connection.py` lines 1042–1056). A probe showed two effects on the shared context:

- After one `verify=True` request through an adapter created with `assert_hostname=False`, `_preloaded_ssl_context.check_hostname` is `False` for the rest of the process. Nothing sets it back.
- Once that has happened, the request from finding 3 no longer raises. It returns 200 and leaves `_preloaded_ssl_context.verify_mode == ssl.CERT_NONE`.

Two further probes bounded the impact. Hostname mismatches are still rejected after the flip, because urllib3 falls back to its own hostname check. The next `verify=True` connection sets `verify_mode` back to `CERT_REQUIRED` before wrapping its socket.

**Failure.** The exposure is a race, not a persistent downgrade. If one thread makes an HTTP-via-HTTPS-proxy request between another thread's `verify_mode` assignment and its socket wrap, that other thread's `verify=True` handshake runs with `CERT_NONE`, and urllib3 then also skips its hostname check because it reads the same field. The window is narrow and was not reproduced. The preconditions are a prior adapter using `assert_hostname` or `assert_fingerprint`, or pyOpenSSL injection, where `check_hostname` is always off. Not verified here: urllib3 1.26 creates contexts with `check_hostname=False`, which would remove the precondition and replace the `ValueError` in finding 3 with this behaviour.

### 7. [Low] `cert_verify()` no longer configures trust for `verify=True` on pools it did not expect

**Where:** `src/requests/adapters.py` lines 297–317.

**Evidence.** `cert_verify` and `get_connection` are both documented as exposed for subclasses. The new `cert_verify` assumes the pool already carries the preloaded context, which is only true for pools built by `_urllib3_request_context`. A probe on a pool from the public `get_connection()` showed:

- After `cert_verify(pool, url, True, None)` the pool has no `ssl_context` and `ca_certs` is `None`, so urllib3 will use the OS store. At the base it was the certifi path.
- After `cert_verify(pool, url, "<ca path>", None)` followed by `cert_verify(pool, url, True, None)` on the same pool, `ca_certs` is still the custom path. At the base the second call replaced it with certifi.

**Failure.** Subclasses that override `send()` or otherwise pair `get_connection()` with `cert_verify()` silently change trust store for `verify=True`: the OS store on a fresh pool, or whatever bundle an earlier request left on a reused pool. On hosts without an OS store those requests fail verification. The built-in `send()` is not affected, since it uses `_get_connection`.

## Checked and not reported

- The new `ca_cert_dir` branch for a directory passed as `verify` (lines 96–100) is correct and fixes a base inconsistency where the directory was also sent as `ca_certs`.
- Plain `http://` requests without a proxy are unaffected: urllib3 strips `ssl_context` from HTTP pool arguments.
- Import cost of preloading the bundle measured about 3 ms on this machine, too small to report.
- Running from a zip archive still works: `extract_zipped_paths` is applied, now once at import.
