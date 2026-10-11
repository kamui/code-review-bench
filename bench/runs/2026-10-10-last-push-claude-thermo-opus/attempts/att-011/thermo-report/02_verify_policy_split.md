# 02 — The `verify` policy is split across two functions

Scope: `src/requests/adapters.py:91-101` (`_urllib3_request_context`) and
`src/requests/adapters.py:297-321` (`HTTPAdapter.cert_verify`). Carries the evidence for
summary findings F3, F4, F6, F7 and F8.

## The shape after the diff

Two functions now translate `verify` into urllib3 trust configuration, and they run back to
back on the same pool in `send` (lines 532 and 536):

| | `_urllib3_request_context` (private, new-ish) | `cert_verify` (public subclass hook) |
| --- | --- | --- |
| "don't verify" | `verify is False` | `not verify` (also `None`, `0`, `""`) |
| "default bundle" | `verify is True` → shared context | `verify is True` → set nothing |
| "caller's bundle" | `isinstance(verify, str)` → `isdir` → `ca_certs` / `ca_cert_dir` | anything else → `exists` check → `isdir` → `ca_certs` / `ca_cert_dir` |
| anything else | falls through, `CERT_REQUIRED`, no anchors | treated as a path |

The diff adds a second copy of the `os.path.isdir` fork (lines 97-100, duplicating lines
314-317) and a third arm to the `verify` ladder, and it makes the two functions depend on
each other: `cert_verify`'s `verify is True` branch is only correct if the pool it is
handed was created by `_urllib3_request_context` with the shared context inside it. That
dependency is stated in a comment (lines 300-303) and enforced nowhere. The author says as
much in the thread ("I believe this is now fully redundant with the corresponding logic in
`cert_verify()`") and in the PR body ("I'm not sure that setting `conn.ca_certs` ... in
`cert_verify()` is even still needed"). The duplication was noticed and shipped anyway.

Credit where due: the new `isdir` fork fixes a merge-base defect. At the base,
`_urllib3_request_context` put a directory into `ca_certs`, and `cert_verify` then set
`ca_cert_dir` without clearing `ca_certs`, so a CA directory reached urllib3 as both
`cafile` and `capath`. That fix is worth keeping; it just should not exist twice.

## F3 — `cert_verify` is no longer self-sufficient (verified)

`cert_verify` is documented in its own docstring as "exposed for use when subclassing", and
`get_connection` (lines 406-433) is still public and still returns a pool with no TLS
kwargs. A subclass that overrides `send` using the long-standing recipe
`conn = self.get_connection(url, proxies); self.cert_verify(conn, url, verify, cert)` now
gets a pool with `CERT_REQUIRED` and no trust anchors at all.

Probe `test_scratch_probe.py::test_probe_C_cert_verify_no_longer_self_sufficient`:

```
PROBE_C get_connection()+cert_verify(verify=True): cert_reqs= CERT_REQUIRED ca_certs= None ca_cert_dir= None ssl_context= None
```

With no anchors and no context, urllib3 builds a default context and calls
`load_default_certs()` (`urllib3/connection.py:1061-1068`). So the request does not fail
loudly; it silently verifies against the OS trust store instead of the certifi bundle (or
the bundle extracted from a zipapp). On hosts with no usable OS store it fails with
`CERTIFICATE_VERIFY_FAILED`; on hosts with extra roots in the OS store it trusts more than
requests is documented to trust. At the merge-base the same two calls set
`conn.ca_certs` to the default bundle.

## F4 — the TLS handshake to an HTTPS proxy changes trust store (verified)

This one needs no subclassing. For an `https://` destination tunnelled through an
`https://` proxy, urllib3 performs the proxy handshake in `_connect_tls_proxy`
(`urllib3/connection.py:943-983`). When no `proxy_ssl_context` is configured — requests
never configures one — it uses `ssl_context = self.ssl_context if self.proxy_is_forwarding else None`
and `ca_certs = self.ca_certs`. For a tunnel, that is `ssl_context=None` plus whatever
`cert_verify` put on the pool.

- Merge-base: `cert_verify` set `conn.ca_certs` to the default bundle, so the proxy was
  verified against certifi.
- Head: `cert_verify` sets nothing for `verify is True`, so `ca_certs` is `None`, the
  context is a fresh default one, and `load_default_certs()` runs. The proxy is verified
  against the OS store, and the shared context is not consulted at all.

Probe `test_probe_D_https_proxy_handshake_trust_store`: a local TLS server stands in for the
proxy, with a certificate from a throwaway CA that is not in certifi. `SSL_CERT_FILE` points
at that CA, so it is trusted only via OpenSSL's default-paths mechanism. The request is
`GET https://example.invalid/` with `verify=True` and `proxies={"https": "https://localhost:<port>"}`.

```
PROBE_D client outcome: ProxyError: HTTPSConnectionPool(host='example.invalid', port=443): Max retries exceeded with url: / (Caused by ProxyError('Unable to connect to proxy', OSError('Tunnel conn...
PROBE_D proxy-side records: [(True, 'CONNECT example.invalid:443 HTTP/1.1', None)]
```

The handshake completed and the `CONNECT` line arrived (the stand-in then answers 502 on
purpose, hence the `ProxyError`). A certifi-only client could not have completed that
handshake. The merge-base half of the comparison is by reading — the clone is pinned to the
head and the allowance does not cover running the base — but it follows directly from the
removed lines `cert_loc = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)` /
`conn.ca_certs = cert_loc`.

This also means the PR's optimisation does not reach this path: every new connection
through an HTTPS proxy still builds a context and loads a trust store, just a different one.
The thread flagged proxies as "might not yet be covered by this PR"; they are not covered,
and they are changed.

## F6 — `verify is True` is special-cased instead of modelled

The slow call is `load_verify_locations`, and urllib3 makes it per connection whenever
`ca_certs` or `ca_cert_dir` is set (`urllib3/util/ssl_.py:413-415`). That is equally true
for a caller-supplied bundle. The diff only short-circuits the literal `True`:

```
PROBE_E pool kwargs for verify=<path> (REQUESTS_CA_BUNDLE shape): {'ca_certs': '/etc/ssl/custom-bundle.pem', 'cert_reqs': 'CERT_REQUIRED'}
```

`Session.merge_environment_settings` turns `REQUESTS_CA_BUNDLE` / `CURL_CA_BUNDLE` into a
string `verify`, so every deployment that sets one of those — the standard way to run
requests behind a corporate CA — gets none of the benefit, while still inheriting the new
import-time work. The PR's stated use case is "lots of requests concurrently"; whether it
helps now depends on an environment variable.

The cause is the modelling. The concept that wants to exist is "a trust-store location has a
reusable context"; `True` is merely one location (`DEFAULT_CA_BUNDLE_PATH`). Expressed that
way the three-arm ladder collapses to two steps, resolve then look up, and the special case
disappears:

```python
def _ca_location(verify):
    """The CA file or directory that ``verify`` designates."""
    cert_loc = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH) if verify is True else verify
    if not cert_loc or not os.path.exists(cert_loc):
        raise OSError(
            f"Could not find a suitable TLS CA certificate bundle, "
            f"invalid path: {cert_loc}"
        )
    return cert_loc


def _ca_kwargs(cert_loc):
    """The urllib3 keyword that carries ``cert_loc``; the only isdir() fork."""
    return {"ca_cert_dir" if os.path.isdir(cert_loc) else "ca_certs": cert_loc}


@functools.lru_cache(maxsize=8)
def _verifying_context(cert_loc):
    context = create_urllib3_context()
    context.load_verify_locations(**{
        "capath" if os.path.isdir(cert_loc) else "cafile": cert_loc
    })
    return context
```

Caching by path has one honest trade-off that needs a maintainer decision: a bundle
rewritten on disk during the life of the process is no longer picked up for new
connections. That is already true of the default bundle at the head. If that is not
acceptable for caller-supplied paths, keep the cache to the default location only — but
then say so in a comment and keep `_ca_location` / `_ca_kwargs` anyway, because they are
what removes the duplication.

## F7 — worked consolidation of the two functions

With the helpers above and `_shared_ssl_context` from `01_shared_context_ownership.md`, the
decision is made once and `cert_verify` reads the result off the pool instead of
re-deriving it from `verify`:

```python
# _urllib3_request_context
if verify is False:
    pool_kwargs["cert_reqs"] = "CERT_NONE"
else:
    pool_kwargs["cert_reqs"] = "CERT_REQUIRED"
    shared = _shared_ssl_context(verify, client_cert, manager)
    if shared is not None:
        pool_kwargs["ssl_context"] = shared
    elif verify is True or isinstance(verify, str):
        pool_kwargs.update(_ca_kwargs(_ca_location(verify)))
```

```python
# cert_verify
if url.lower().startswith("https") and verify:
    conn.cert_reqs = "CERT_REQUIRED"
    # Trust anchors travel either in the shared context or in ca_certs/ca_cert_dir.
    if conn.conn_kw.get("ssl_context") is not _default_ssl_context():
        for name, value in _ca_kwargs(_ca_location(verify)).items():
            setattr(conn, name, value)
else:
    ...
```

What this buys:

- `cert_verify` is self-sufficient again: a pool from `get_connection`, or one carrying an
  adapter's own context, gets `ca_certs` exactly as at the merge-base (F3).
- The invariant "this pool already has its anchors" is a check on the pool, not a comment
  about what another function probably did.
- One `isdir` fork, one existence check, one place that knows `True` means
  `DEFAULT_CA_BUNDLE_PATH`.
- F4 is fixed by adding one exclusion to `_shared_ssl_context` — do not share when the
  manager is an HTTPS `ProxyManager`, because the proxy handshake reads `ca_certs` and
  ignores `ssl_context` — after which `cert_verify` sets `ca_certs` and the merge-base trust
  store is restored. (If verifying proxies against the OS store is actually wanted, that is
  a separate, documented change; see the questions in the summary.)

The identity comparison in `cert_verify` calls `_default_ssl_context()`, which builds the
context if it does not exist yet; if that is unwanted on the `get_connection` path, compare
against a module-level sentinel holder instead. That is a detail; the structure is the point.

## F8 — no tests

The diff is +28/−18 in one source file with no test changes. The author's stated reason is
that they "couldn't find a way to access such low-level information about a request in the
exposed classes." The information is one attribute away: `_urllib3_request_context` is a
pure function returning a dict, and `HTTPAdapter._get_connection(...)` returns a pool whose
`conn_kw` shows exactly which context and CA kwargs were chosen (the probes in
`clone-work/scratch/` do precisely this). At minimum the change should pin:

1. `verify=True`, no client cert → pool carries the shared context and no `ca_certs`.
2. `verify=True` with `cert=` → pool does **not** carry the shared context.
3. Adapter with its own `ssl_context` / `ssl_version` → pool kwargs unchanged by `verify=True`.
4. `verify=<file>` → `ca_certs`; `verify=<dir>` → `ca_cert_dir` only (the base defect).
5. `get_connection()` + `cert_verify(verify=True)` → `conn.ca_certs` is the default bundle.

## Existing suite

`PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=$PWD/src ../clone-cache/venv/bin/python -m pytest -p no:cacheprovider -q tests/test_adapters.py tests/test_requests.py -k "verify or cert or ssl or tls or adapter or pool or proxy"`
→ 38 passed, 3 failed, 289 deselected.

- `test_pyopenssl_redirect` and
  `test_different_connection_pool_for_tls_settings_verify_bundle_unexpired_cert` are the two
  failures the run policy lists as present at the merge-base as well.
- `test_different_connection_pool_for_mtls_settings` also fails, on its first request
  (`s.get(url, verify=False, cert=cert)`, `tests/test_requests.py:2959`) with
  `SSLV3_ALERT_CERTIFICATE_EXPIRED` sent by the test server. The fixture client certificate
  `tests/certs/mtls/client/client.pem` has `notAfter=Mar 13 18:35:04 2026 GMT` and the run
  date is 2026-10-10. The failing request takes the `verify is False` arm, which the diff
  does not touch. I attribute this to fixture expiry, not to the change; it was not run at
  the merge-base, so that attribution rests on the expiry date and the code path.

None of the 38 passing tests exercises any of the behaviours in F1-F4, which is the
practical meaning of "no tests".

## Verification status

| Claim | Status |
| --- | --- |
| `get_connection` + `cert_verify(verify=True)` leaves no trust anchors | Verified by execution (PROBE_C) |
| That state leads to `load_default_certs()` | Read from `urllib3/connection.py:1061-1068` |
| HTTPS-proxy handshake trusts the OS default store at the head | Verified by execution (PROBE_D) |
| HTTPS-proxy handshake used certifi at the merge-base | Read from the removed lines; base not executed |
| `verify=<path>` gets no shared context | Verified by execution (PROBE_E) |
| Merge-base passed a CA directory as both `ca_certs` and `ca_cert_dir` | Read from the diff; not executed |
| File size | 606 → 616 lines (`wc -l`); no decomposition concern |
