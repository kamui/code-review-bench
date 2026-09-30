# 01 — `src/requests/adapters.py`: TLS trust policy after psf/requests#6667

Review range: `8dd3b26bf59808de24fd654699f592abf6de581e..4089f3dc65f783beaa53cc032958ab625440d0ac`
(`git diff main...review-head`, one file, +28 / −18). File size goes from 606 to 616 lines,
so the 1k-line threshold does not come into play.

All line numbers refer to `review-head`.

## Environment and commands

- Interpreter: the pre-provisioned venv in the attempt's cache (Python 3.13, urllib3 2.8.0).
- Scratch probes live in `clone-work/scratch/` (`probe.py`, `probe_base.py`, `nossl.py`,
  `missing.py`). The merge-base tree was exported with `git archive main src | tar -x` into
  `clone-work/scratch/base/`, so probes could run against base and head side by side.
  Nothing was written into the clone; `git status --short` stayed empty.
- Focused tests at head:
  `PYTHONPATH=src venv/bin/python -m pytest tests/test_requests.py -k "ssl or verify or cert or connection_pool" -q -p no:cacheprovider`
  → 15 passed, 3 failed. The two known fixture-certificate failures
  (`test_pyopenssl_redirect`, `test_different_connection_pool_for_tls_settings_verify_bundle_unexpired_cert`)
  showed up as expected. The third, `test_different_connection_pool_for_mtls_settings`, fails
  with the same `SSLV3_ALERT_CERTIFICATE_EXPIRED` error when run against the merge-base tree,
  so it is an environment problem (expired fixture cert), not something this change caused.
  None of the passing tests exercise the new `ssl_context` branch in a way that would catch
  the findings below.

---

## Finding 1 — Verify policy is now split across two layers, with duplicated dispatch and a half-dead `cert_verify`

**Where:** `_urllib3_request_context` lines 91–100 and `HTTPAdapter.cert_verify` lines 297–317.

**Verification status:** confirmed by reading the code. The duplication and the per-connection
reload for string bundles follow directly from urllib3's `ssl_wrap_socket`
(`urllib3/util/ssl_.py:407–415`: it calls `load_verify_locations` whenever `ca_certs` or
`ca_cert_dir` is set on the connection). The `REQUESTS_CA_BUNDLE` path is confirmed in
`sessions.py:768–771`.

Before this PR, `cert_verify` was the single place that turned `verify` into trust settings
on the pool: resolve the default bundle, check that it exists, then pick `ca_certs` or
`ca_cert_dir`. The PR moves half of that decision into `_urllib3_request_context`. There,
`verify is True` now means "use the module-global context", and a string means "`isdir` →
`ca_cert_dir`, else `ca_certs`". The other half stays in `cert_verify`, which runs the
existence check and then repeats the same `isdir` → `ca_cert_dir` / `ca_certs` dispatch on the
pool object (lines 314–317 copy lines 97–100 almost verbatim).

As a result, a reader has to follow two functions and one cross-function invariant to know
what trust store a request uses. The new comment at lines 300–303 states that invariant
outright: "the connection will be using a context with the default certificates already
loaded". That is only true if the pool came from `_get_connection` with the unmodified
`_urllib3_request_context` behind it. Nothing enforces it, and Finding 2 shows one ordinary
way it breaks. The comment is also wrong about its own branch: it says "if verify is a
boolean", but `verify=False` never reaches this code. And `# verify must be a str with a path
then` is an unchecked type assumption dressed up as a comment. `cert_loc = verify` is now a
pointless alias.

The split also limits the performance fix to the literal `verify=True` case. With a custom
bundle, both layers still set `ca_certs` / `ca_cert_dir`. urllib3 builds a fresh context per
connection and calls `load_verify_locations` each time, which is the slow path the PR set out
to remove. This covers every user with `REQUESTS_CA_BUNDLE` or `CURL_CA_BUNDLE` set:
`Session.merge_environment_settings` turns `verify=True` into that path string before the
adapter sees it. That is a common corporate setup, and those users get no benefit. The PR body
admits the logic "could be moved to `_urllib3_request_context()` and benefit from using a
cached context in those cases too". That move is the code-judo step, and it deletes code
instead of adding it.

### Worked code-judo proposal

Make one function own "what does `verify` mean for the pool", and have it return a context
for every verifying case. Then `cert_verify` has no trust-store logic left to duplicate.

```python
@functools.lru_cache(maxsize=None)
def _ssl_context_for_ca(ca_path: str) -> "ssl.SSLContext":
    """Build (once per path) a verifying context with the given CA bundle/dir loaded."""
    if not os.path.exists(ca_path):
        raise OSError(
            f"Could not find a suitable TLS CA certificate bundle, invalid path: {ca_path}"
        )
    ctx = create_urllib3_context()
    if os.path.isdir(ca_path):
        ctx.load_verify_locations(capath=ca_path)
    else:
        ctx.load_verify_locations(cafile=ca_path)
    return ctx


def _tls_pool_kwargs(verify) -> dict:
    if verify is False:
        return {"cert_reqs": "CERT_NONE"}
    ca_path = verify if isinstance(verify, str) else extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
    return {"cert_reqs": "CERT_REQUIRED", "ssl_context": _ssl_context_for_ca(ca_path)}
```

`_urllib3_request_context` then calls `pool_kwargs.update(_tls_pool_kwargs(verify))` in
place of its `if/elif/elif` ladder. `cert_verify` shrinks to the client-cert (`cert`)
handling plus a compatibility path for pools that did not come from `_get_connection`. Those
are legacy `get_connection` overrides, and the old `ca_certs` assignment can stay there.

What this buys: the `isdir` dispatch exists once; the existence check and its curated
`OSError` exist once; custom bundles and `REQUESTS_CA_BUNDLE` get the same cached-context
speedup; and the `verify is True` special case goes away, because the default bundle is just
another path. Caveat: an `lru_cache` keyed by path does not notice edits to the file after
first use. That is a real semantic choice and should be decided on purpose, e.g. by keying on
`(path, mtime)`. The current PR already makes the same trade-off for the default bundle
without saying so.

---

## Finding 2 — The `verify is True` branch overrides a subclass's `ssl_context`, breaking the documented `init_poolmanager` extension point

**Where:** `_urllib3_request_context` lines 94–95, reached from `_get_connection` lines 377–404.

**Verification status:** confirmed by probe (`scratch/probe.py` at head, `scratch/probe_base.py`
at the merge-base).

`HTTPAdapter.init_poolmanager` and `proxy_manager_for` are documented as "exposed for use when
subclassing". The common, long-recommended pattern for custom TLS (pinned ciphers, a
system/truststore context, mTLS on the context, legacy renegotiation, and so on) is to
override `init_poolmanager` and pass `ssl_context=...` into the `PoolManager`. urllib3 merges
per-request `pool_kwargs` over the manager's `connection_pool_kw`. So the new unconditional
`pool_kwargs["ssl_context"] = _preloaded_ssl_context` replaces the subclass's context on the
default `verify=True` path, silently and with no error.

Probe output at head, with an adapter whose `init_poolmanager` sets a custom context:

```
True  pool ssl_context is custom: False | is preloaded: True
False pool ssl_context is custom: True  | is preloaded: False
'<bundle path>' pool ssl_context is custom: True | is preloaded: False
```

At the merge-base, all three cases report `custom: True`. This is the "ad-hoc special case
inserted into a shared path" failure mode: an optimisation meant for the default configuration
was placed where every configuration flows through it. The upshot is that `verify=True` (the
default!) is now the one setting where a subclass's TLS configuration is ignored. A user's
cipher or trust policy can be dropped without any signal, so this is a security-relevant
behaviour change, not only a structural one.

**Remedy.** The default context has to be a fallback, not an override. The smallest fix is to
inject it only when the manager does not already carry a context. That needs the manager in
scope, so `_get_connection` should pass `manager.connection_pool_kw` (or a boolean) into the
helper from Finding 1, instead of the helper blindly writing `ssl_context`. Cleaner still,
Finding 1's `_tls_pool_kwargs` is where this rule belongs:
`if "ssl_context" in manager_kwargs: return {"cert_reqs": ...}`. Pair it with a regression test
modelled on `test_different_connection_pool_for_tls_settings_verify_True`
(`tests/test_requests.py:2833`) that asserts `conn.conn_kw["ssl_context"] is custom`.

---

## Finding 3 — Import-time global side effect: import breaks without `ssl`, the curated missing-bundle error is lost, and `DEFAULT_CA_BUNDLE_PATH` is frozen at import

**Where:** module level, lines 75–78 (`_preloaded_ssl_context = create_urllib3_context()` then
`load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))`), plus the deleted guard
that used to live in `cert_verify` (`if not cert_loc or not os.path.exists(cert_loc): raise OSError(...)`).

**Verification status:** confirmed by probes at head versus the merge-base.

This moves work that used to be lazy and per-request into `import requests`. That has
consequences beyond performance, and each one below was checked.

1. **No-`ssl` interpreters can no longer import requests.** `scratch/nossl.py` blocks the `ssl`
   import. At the merge-base: `import ok`. At head:
   `import failed: TypeError Can't create an SSLContext object without an ssl module`. requests
   has always been importable (and usable over plain HTTP) on builds without `ssl`. urllib3
   explicitly supports that and turns the `ssl` import failure into a runtime error.
2. **The missing/unreadable-bundle error regresses from a clear `OSError` at request time to a
   bare `FileNotFoundError` at import time.** `scratch/missing.py` points `certifi.where()` at a
   nonexistent file. Base: `import ok`, then
   `OSError Could not find a suitable TLS CA certificate bundle, invalid path: /nonexistent/cacert.pem`.
   Head: `FileNotFoundError [Errno 2] No such file or directory`, raised by `import requests`
   itself. Broken freezer/zipapp packaging (the case sigmavirus24 cared about in review) now
   fails at import with no path in the message, and even for programs that never make an HTTPS
   request.
3. **`DEFAULT_CA_BUNDLE_PATH` is snapshotted.** `cert_verify` used to read the module global on
   every call, so reassigning `requests.adapters.DEFAULT_CA_BUNDLE_PATH` (a workaround used by
   some packagers and test suites) took effect. The probe reassigns it after import and gets
   `patched default honored? False`.
4. `extract_zipped_paths` now runs at import for zipped deployments, writing the bundle into a
   temp directory even when no request is ever made.

A related, lower-severity note: urllib3 writes `context.verify_mode = resolve_cert_reqs(cert_reqs)`
onto the supplied context for every connection (`urllib3/connection.py:1040–1042`). For the
shared global this is idempotent, because only `verify=True` pools use it and they always pass
`CERT_REQUIRED`. It does mean the "never reconfigure a shared context" rule (tiran's comment)
holds only by coincidence of how the pools are keyed. That is one more reason to keep ownership
of this object in a single, clearly documented helper.

**Remedy.** Replace the module-level statement with a lazy, memoised accessor, e.g.
`_ssl_context_for_ca()` from Finding 1 or at minimum a `functools.lru_cache`'d
`_default_ssl_context()`. It should run the existence check with the original `OSError`
message before loading, and it should read `DEFAULT_CA_BUNDLE_PATH` at call time. That keeps
all of the performance win (one load per process per bundle), restores lazy failure with a
useful message, and keeps `import requests` free of TLS side effects.

---

## Finding 4 — No test pins the new pool/context contract, although the needed hook is easy to reach

**Where:** the PR adds no tests. The existing `test_different_connection_pool_for_tls_settings_*`
tests in `tests/test_requests.py:2833–2960` only assert that pools are distinct.

**Verification status:** confirmed. `scratch/probe.py` reads the context straight off
`HTTPAdapter()._get_connection(prepared, verify).conn_kw["ssl_context"]`.

In review, the author said they could not find a way to reach the context used by a request.
The pool built by `_get_connection` exposes it in `conn_kw`, and the existing TLS pool tests
already call `_get_connection`. The findings above show why tests are needed: the invariants
this PR relies on are that `verify=True` uses the shared preloaded context, that custom bundles
never touch it, and that a subclass context wins. None of these are checked, and Finding 2
violates one of them. Add three small tests that assert on `conn.conn_kw` / `conn.ca_certs` for
`verify=True`, `verify=<bundle>`, and a subclass with `init_poolmanager(ssl_context=...)`.
Add one more that `import requests` does not construct an `SSLContext`, once Finding 3 is
fixed.
