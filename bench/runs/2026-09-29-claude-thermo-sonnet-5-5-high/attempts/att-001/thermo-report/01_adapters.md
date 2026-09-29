# Detail report: `src/requests/adapters.py`

Scope: the whole PR (1 file, +28/-18, 4 commits, range `8dd3b26b..4089f3dc`). File size after the change is 616 lines, so the 1k-line threshold is not a concern.

## Method and verification status

Read `git diff main...review-head`, read the surrounding `adapters.py` (`_urllib3_request_context`, `cert_verify`, `_get_connection`, `send`) and read the installed urllib3 2.8.0 source (`util/ssl_.py::ssl_wrap_socket`, `connection.py::_ssl_wrap_socket_and_match_hostname`) to see what urllib3 does with a caller-supplied `ssl_context`. No tests were executed: the claims below are established by reading code paths, not by a running reproduction. Each finding is marked CONFIRMED (code path read end to end) or PLAUSIBLE.

## Finding 1 — the shared module-level SSLContext is mutated by urllib3 per connection (CONFIRMED by code reading)

Location: `src/requests/adapters.py` lines 75-78 (context creation) and 94-95 (`pool_kwargs["ssl_context"] = _preloaded_ssl_context`).

Evidence: urllib3's `ssl_wrap_socket` does `context = ssl_context` and then, when `certfile` is set, calls `context.load_cert_chain(certfile, keyfile)` on that same object (`util/ssl_.py` lines ~429-433). `_ssl_wrap_socket_and_match_hostname` also does `context.verify_mode = resolve_cert_reqs(cert_reqs)` (`connection.py` ~1042) and may set `context.check_hostname = False` (~1056) on the caller's context. In requests, `_urllib3_request_context` sets `cert_file`/`key_file` in `pool_kwargs` from `cert=`, and now also sets `ssl_context` to the process-wide object whenever `verify is True`. Nothing in the diff guards the combination.

Consequence: a single `requests.get(url, cert=("client.pem", "client.key"))` with default `verify=True` loads that client certificate and key into the process-global context. Every later verified connection in the process, from any session, any adapter, any host, would then present that identity during handshakes where the server requests a client certificate, and multiple different client certs accumulate in the same context. That is a cross-request state leak and a potential identity-disclosure problem, and it is exactly the "don't reconfigure the context once used" caveat that the discussion in the PR raised (tiran: adding to the trust store is fine, changing verification settings or mTLS certs is not). The rename to `_preloaded_ssl_context` made mutation less tempting for users but did nothing about the library itself mutating it. Additionally, `verify_mode` assignment on a shared object across threads is the kind of write the PR's concurrency premise was trying to avoid depending on.

Worked remedy: do not share the context when a client cert is involved (`if client_cert is None: pool_kwargs["ssl_context"] = shared` else fall back to `ca_certs=DEFAULT_CA_BUNDLE_PATH`), or better, build one immutable-by-convention context per distinct configuration, cached, with `load_cert_chain` applied before caching (see the code-judo proposal below). At minimum add a test that sends with `cert=` and `verify=True` and asserts the module context's cert chain is unchanged.

## Finding 2 — import-time side effect and lost lazy failure mode (CONFIRMED)

Location: `src/requests/adapters.py` lines 75-78; removed check at former `cert_verify` lines 300-312.

Evidence: `create_urllib3_context()` plus `load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))` now run at `import requests`. Before, the bundle was resolved and validated lazily, and a missing bundle raised `OSError("Could not find a suitable TLS CA certificate bundle, invalid path: ...")` from `cert_verify` at request time. That check is deleted for the `verify=True` path (the diff only keeps it under `verify is not True`).

Consequences: (a) every importer pays the OpenSSL bundle-load cost (the very cost the PR calls slow, on the order of 20 ms per load) and the zip extraction side effect of `extract_zipped_paths`, even if it only ever uses `verify=False`, plain HTTP, or a custom bundle, or is just importing `requests.exceptions`; (b) an environment where certifi's bundle is missing, stripped by a packager, or unreadable now fails with a raw `FileNotFoundError`/`ssl.SSLError` at import time instead of the documented per-request `OSError`, breaking `import requests` for callers that never use TLS; (c) the friendly error message for the default bundle is gone. The reviewers' PR discussion explicitly cared about zipped installs (pants, pyinstaller); the extraction now also happens eagerly at import.

Worked remedy: make the context lazy behind a small `functools.lru_cache`d function (`_default_ssl_context()`), which keeps the once-per-process cost, removes the import-time cost and failure, and gives a single place to keep the "could not find CA bundle" error. That also gives the code a natural home for a lock-free, thread-safe lazy init.

## Finding 3 — CA-location dispatch is duplicated across two functions that must agree (CONFIRMED; code-judo opportunity)

Location: `_urllib3_request_context` lines 91-100 and `cert_verify` lines 298-317.

Evidence: the same decision (`verify` True / str / dir vs file → `ca_certs` vs `ca_cert_dir`) is now written twice: once into `pool_kwargs` (which selects and keys the pool and is what urllib3 uses to construct the pool) and once onto the connection object in `cert_verify` (which mutates the pooled connection *after* it was selected). The PR author says in the description that setting `conn.ca_certs` in `cert_verify()` may no longer be needed. Because pools are already keyed by `ca_certs`/`ca_cert_dir`, the per-connection assignment in `cert_verify` is redundant for str `verify` and is the only reason `cert_verify` still has the `isdir` branch. The diff added a second `os.path.isdir` and a second `if/elif` chain rather than deleting the first. The comment in `cert_verify` ("`verify` must be a str with a path then") also documents a type assumption instead of enforcing it; `Path` objects or other truthy non-str values pass `cert_verify` but are silently ignored by `_urllib3_request_context` (`isinstance(verify, str)`), so the two halves can disagree.

Code-judo proposal: make `_urllib3_request_context` the single source of truth. Compute `verify` → `{ssl_context | ca_certs | ca_cert_dir, cert_reqs}` there (isdir decision once, existence check once, including the friendly OSError), and reduce `cert_verify` to `conn.cert_reqs`/`ca_*` handling only for the `verify=False` reset and the client-cert assignment, or make it a deprecated hook that just validates. That deletes the second isdir branch and the `verify is not True` nesting, and it also removes the need for the multi-line comment block. The result is a flat function with three arms (`False`, `True`, `path`) and no duplicate state on the connection.

## Finding 4 — ordering and nesting of the new branches in `_urllib3_request_context` (CONFIRMED, minor)

Location: lines 91-100.

Evidence: the function initialises `cert_reqs = "CERT_REQUIRED"`, overrides it in an `if verify is False` and then chains `elif verify is True` / `elif isinstance(verify, str)` with a nested `if not isdir … else …`. The negated `if not os.path.isdir(verify): … else:` reads backwards. The ternary key choice (`"ca_certs" if not isdir else "ca_cert_dir"`) is a one-liner. A `verify` value that is neither bool nor str (e.g. `None`, a `Path`) now falls through with neither `ssl_context` nor `ca_certs` set, which yields a pool with no explicit trust configuration (urllib3 loads OS default certs in some paths) — a silent fallback rather than an explicit invariant.

Remedy: extract `_ca_pool_kwargs(verify)` returning a dict, handle `Path` via `os.fspath`, and raise or normalise for unknown types.

## Finding 5 — no tests accompany a change to certificate-trust behaviour (CONFIRMED)

The diff touches only `adapters.py`. There is no test that `verify=True` still trusts certifi's bundle and rejects an untrusted cert via the new shared context, none that a custom bundle path and a directory path route to `ca_certs`/`ca_cert_dir`, and none for the client-cert interaction in Finding 1. The author noted in review that they could not find a way to inspect the loaded certificates; that is itself a signal the seam is hard to test, which supports the lazy-cached-function proposal (a function returning the context can be asserted on directly with `cert_store_stats()`).

## Things checked and found acceptable

- File size stays well under 1k lines.
- The shared-context approach is compatible with pool keying: urllib3 folds `ssl_context` into the pool key (`key_ssl_context`), so the same object gives cache hits, which is the intended performance win.
- Naming was already made private in commit 3; the last commit is lint only.
