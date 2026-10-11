# 02 — Trust-anchor ownership, import-time construction, duplication (findings 4, 5, 6, 7)

Subject: `src/requests/adapters.py` at head `4089f3dc`. Environment and probe invocation are described at the top of `01_shared_context_state.md`.

## Measurements

```
$ wc -l src/requests/adapters.py            -> 616
$ git show main:src/requests/adapters.py | wc -l   -> 606
$ git diff main...review-head --stat        -> 1 file changed, +28 / -18
```

No file-size concern. The diff adds one module-level object, one `elif`, one nested `if/else`, and re-indents the body of `cert_verify` under a new guard.

## Finding 4 — `cert_verify` relies on an unchecked invariant

Head, lines 297-317, condensed:

```python
if url.lower().startswith("https") and verify:
    conn.cert_reqs = "CERT_REQUIRED"
    # ... the connection will be using a context with the default certificates already loaded ...
    if verify is not True:
        ...validate path, set conn.ca_certs / conn.ca_cert_dir
```

Merge-base, same place: `cert_loc` defaulted to `extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)`, was checked with `os.path.exists`, and was always assigned. `cert_verify` was self-sufficient: given any pool, it left the pool able to verify.

On the head the method is correct only if its caller obtained `conn` from `_get_connection` and the handshake in question is the one to the target host. Neither is enforced or checked.

**Legacy pairing (probe 6).** `get_connection` remains public at lines 406-433 and builds pools without `pool_kwargs`.

```python
a = HTTPAdapter()
conn = a.get_connection("https://example.invalid/")
a.cert_verify(conn, "https://example.invalid/", True, None)
```

```
PROBE6 get_connection()+cert_verify(verify=True): cert_reqs='CERT_REQUIRED' ca_certs=None ca_cert_dir=None ssl_context=None
```

With that state urllib3 takes the `default_ssl_context` branch and calls `load_default_certs()` (`connection.py:1063-1068`). Subclasses that reproduce the older `send` body, or that override `send` and call these two hooks, move from certifi to the OS store, and fail where the interpreter has no usable one.

**HTTPS proxy tunnel (probe 5).** urllib3 keeps the pool's context away from the proxy handshake unless the proxy is forwarding, and uses the pool's CA attributes instead:

```python
963            # Otherwise we inherit the pool's cert policies
964            ssl_context = self.ssl_context if self.proxy_is_forwarding else None
965            cert_reqs = self.cert_reqs
966            ca_certs = self.ca_certs
967            ca_cert_dir = self.ca_cert_dir
```

The probe replaces `_ssl_wrap_socket_and_match_hostname` with a recorder and requests an HTTPS URL through `https://127.0.0.1:P`:

```
PROBE5 TLS-to-proxy wrap kwargs: {'cert_reqs': 'CERT_REQUIRED', 'ca_certs': None, 'ca_cert_dir': None, 'ssl_context': None, 'server_hostname': '127.0.0.1'}
```

On the merge-base `ca_certs` here was the certifi path, because `cert_verify` had assigned it to the pool. The probe shows the arguments; it does not complete a handshake, so the claim "verified against the OS store" is the recorded arguments plus the `load_default_certs()` branch.

**Lost diagnosis.** The `OSError("Could not find a suitable TLS CA certificate bundle, invalid path: …")` for the default bundle is gone (it is now reachable only for a user-supplied path, lines 308-312). See finding 5 for what replaces it.

Remedy. Two lines turn the comment into a check:

```python
has_loaded_context = conn.conn_kw.get("ssl_context") is not None
if not (verify is True and has_loaded_context):
    cert_loc = verify if verify is not True else extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
    ...existing validation and assignment, as on the merge-base...
```

That restores the legacy pairing. It does not restore certifi for the HTTPS-proxy handshake, because assigning `ca_certs` on a pool that also holds the shared context would make urllib3 call `load_verify_locations` on the shared context again (`util/ssl_.py`, `if ca_certs or ca_cert_dir or ca_cert_data:`), which is the cost the PR exists to remove. The clean choices are to not use the shared context when the selected proxy is HTTPS, or to give the proxy manager its own `proxy_ssl_context`. Which one depends on the answer to the first open question in the summary.

## Finding 5 — import-time construction

Lines 75-78 execute on `import requests` (`__init__` → `api` → `sessions` → `adapters`).

Probe 8 timed the same two statements in isolation and then reloaded `requests.adapters` with `requests.utils.DEFAULT_CA_BUNDLE_PATH` pointing at a missing file (restoring and reloading afterwards):

```
PROBE8 building the preloaded context costs 3.4 ms
PROBE8 importing requests.adapters with a missing default bundle raised: FileNotFoundError - [Errno 2] No such file or directory
```

3.4 ms is this machine with OpenSSL 3.5.8 and is small; the PR body's own profile on OpenSSL 3.0.2 shows about 23 ms per call under contention (0.681 s over 30 calls). Whatever the figure, it is now paid by every importer, including the ones that only want `requests.exceptions` or only speak plain HTTP. The failure mode matters more than the cost: a raw `FileNotFoundError` with no path in the message, raised from `import requests`, replaces a descriptive error raised from the request that needed the bundle.

Probe 7 patches `requests.adapters.DEFAULT_CA_BUNDLE_PATH` to a nonexistent path and runs `_get_connection` + `cert_verify` for `verify=True`: nothing is raised and the patched value has no effect. On the merge-base, `cert_verify` read that name per request and would have raised.

The `ssl`-less interpreter case is read, not run: `create_urllib3_context` begins with `if SSLContext is None: raise TypeError(...)` (`util/ssl_.py:223-224`), and `src/requests/__init__.py:119-124` shows the package is written to import when `ssl` is unavailable.

Remedy:

```python
@functools.lru_cache(maxsize=None)
def _default_ssl_context():
    context = create_urllib3_context()
    context.load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))
    return context
```

Construction, cost and errors move to the first verified HTTPS request; import stays free of I/O; a missing bundle can be reported with the old message at the call site.

## Finding 6 — one decision, two decoders; the worked restructuring

Where the meaning of `verify` is decoded on the head:

| Question | `_urllib3_request_context` | `cert_verify` |
| --- | --- | --- |
| verification off? | `verify is False` (line 92) | `not (url.startswith("https") and verify)` (line 297) |
| default bundle? | `verify is True` (line 94) | `verify is True`, by omission (line 304) |
| custom path? | `isinstance(verify, str)` (line 96) | `verify is not True` (line 304) |
| file or directory? | `os.path.isdir` (lines 97-100) | `os.path.isdir` (lines 314-317) |
| path exists? | not checked | `os.path.exists` (line 308) |

The fourth row is new duplication added by this diff. The first row is the disagreement behind finding 3. The second row is the unchecked hand-off behind finding 4.

Proposed shape. One function owns the mapping; the shared context is used only when nothing else will be applied to it; everything else takes the merge-base path unchanged.

```python
_TLS_SHAPING_KEYS = frozenset({
    "ssl_context", "ssl_version", "ssl_minimum_version", "ssl_maximum_version",
    "assert_hostname", "assert_fingerprint",
})


def _ca_location_kwargs(path):
    return {"ca_cert_dir" if os.path.isdir(path) else "ca_certs": path}


def _tls_pool_kwargs(scheme, verify, client_cert, manager_kw):
    """Single owner of how `verify`/`cert` become urllib3 TLS settings."""
    if verify is False:
        return {"cert_reqs": "CERT_NONE"}

    kwargs = {"cert_reqs": "CERT_REQUIRED"}
    if isinstance(verify, str):
        kwargs.update(_ca_location_kwargs(verify))
    elif (
        scheme == "https"
        and client_cert is None
        and _TLS_SHAPING_KEYS.isdisjoint(manager_kw)
    ):
        # Nothing else will be applied to the context, so it is safe to share.
        kwargs["ssl_context"] = _default_ssl_context()
    ...client_cert -> cert_file / key_file, as today...
    return kwargs
```

`_get_connection` already knows the proxy and therefore which manager will serve the request, so it can pass that manager's `connection_pool_kw`. `cert_verify` keeps its path validation and client-certificate checks, and for CA assignment does exactly one thing: if the pool has no `ssl_context`, assign via `_ca_location_kwargs(default-or-custom path)` as the merge-base did. That is the same helper, so the file/directory split exists once.

What this deletes: the import-time global (finding 5), the `elif verify is True` special branch as a free-standing case, the second `isdir` split, and the comment that stands in for a check (finding 4). What it fixes by construction: findings 1, 2 and 3 are the three terms of the predicate. What it keeps: the PR's performance win for the case the PR body actually measures (`requests.get(url)` with defaults, many threads).

A stricter reading of the skill's "delete rather than centralise" standard would say that a predicate with three exclusions is still a special case. I agree, and the summary notes the alternative that removes it from requests entirely: cache contexts in urllib3 keyed by the declarative `ca_certs` / `ca_cert_dir` configuration, so requests keeps passing a path and owns no TLS state. That is a different PR in a different repository; for this one, the resolver above is the smallest change that makes the optimisation safe.

## Finding 7 — tests

The diff adds none. The probes in `probes/test_probe.py` are deliberately written against attributes the test suite can use: `pool.conn_kw["ssl_context"]`, `pool.ca_certs`, `pool.cert_reqs`. Probe 3 can be ported onto the repository's `TLSServer(mutual_tls=True)` once it is inverted to assert that the third request presents no certificate.

Existing suite, focused selection, run once on the head:

```
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=$PWD/src <cache>/venv/bin/python -m pytest -p no:cacheprovider -q -W ignore \
    tests/test_adapters.py tests/test_requests.py \
    -k "tls_settings or mtls or certificate or ca_cert or env_cert or adapters or pyopenssl or verify"
3 failed, 15 passed, 312 deselected in 2.84s
```

- `test_pyopenssl_redirect` and `test_different_connection_pool_for_tls_settings_verify_bundle_unexpired_cert` fail with `Missing Authority Key Identifier`. The execution policy records both as failing on the merge-base on this machine (fixture certificates versus the installed OpenSSL).
- `test_different_connection_pool_for_mtls_settings` fails on its first request, `s.get(url, verify=False, cert=cert)`, with the server alert `SSLV3_ALERT_CERTIFICATE_EXPIRED`. `openssl x509 -enddate` on `tests/certs/mtls/client/client.pem` gives `notAfter=Mar 13 18:35:04 2026 GMT`, which is in the past on the review date. The failing request uses `verify=False`, a branch this diff does not change, so I do not attribute it to the PR. I did not run the merge-base to confirm.

The practical consequence for this review is that the repository's only mutual-TLS test cannot currently exercise the `verify=True` + `cert` path where finding 1 lives, which is why probe 3 generates its own certificates.
