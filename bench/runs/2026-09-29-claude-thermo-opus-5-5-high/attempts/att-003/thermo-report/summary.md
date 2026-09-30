# Thermo-nuclear code quality review — psf/requests#6667

Range: `8dd3b26b..4089f3dc` (`main...review-head`), one file: `src/requests/adapters.py` (+28 / −18, now 616 lines, so the 1k-line rule does not apply).

## Verdict

**Not approvable as written.** The goal is right: stop calling `load_verify_locations()` on every request. The mechanism is not. The PR adds a process-wide mutable `SSLContext` at the lowest layer of the adapter module and hands it to urllib3. urllib3 writes to caller-supplied contexts as part of normal operation. The result is two confirmed correctness regressions: client certificates leak across unrelated requests, and adapter-configured `ssl_context`s are silently overridden. It also leaves the meaning of `verify` split across two functions with copy-pasted dispatch and disagreeing type checks, and it moves the expensive work (and a new failure mode) to import time. A cleaner version exists that keeps the whole performance win. It makes the adapter the owner of the "may this request share the default context?" decision, builds the context lazily, and gives `verify` a single decision table.

The focused test selection passes apart from the two failures that also occur at the merge-base (21 passed, 2 failed). The PR adds no tests, and none of the existing tests exercise what it changed.

## Findings

### 1. The shared module-global context is written to by urllib3, so a client certificate from one request is presented on every later `verify=True` connection in the process (CONFIRMED)

`_urllib3_request_context` puts `_preloaded_ssl_context` (`src/requests/adapters.py:75-78`, injected at `:94-95`) into `pool_kwargs` next to `cert_file`/`key_file` whenever `verify=True`. On connect, urllib3's `ssl_wrap_socket` calls `context.load_cert_chain(certfile, keyfile)` on whatever context it is given. `_ssl_wrap_socket_and_match_hostname` also sets `verify_mode` on it every time and sets `check_hostname = False` whenever `assert_hostname`/`assert_fingerprint` is in play. A local probe with no network confirmed that after one `cert=`-bearing request, the module global itself had the client chain loaded. A second pool with `assert_hostname` set flipped the global's `check_hostname` from `True` to `False`. So one Session's mTLS identity is offered to every other host the process talks to, and one host-header-style adapter turns off OpenSSL hostname checking for everybody else. The "SSLContext is thread-safe if you don't reconfigure it" argument in the PR thread is correct, but requests does not control whether it gets reconfigured: its own dependency reconfigures it. The remedy is to hand out the shared context only when nothing will write to it (no client cert, no adapter-supplied context), and to fall back to the merge-base behaviour otherwise. Details, probe output and a worked proposal are in `01_shared_ssl_context_ownership.md`.

### 2. Per-request `ssl_context` now overrides the context an `HTTPAdapter` subclass configures in `init_poolmanager` (CONFIRMED)

`_get_connection` passes `pool_kwargs` to `connection_from_host` (`src/requests/adapters.py:395-402`), and urllib3 merges those *over* `PoolManager.connection_pool_kw`. Subclassing `HTTPAdapter` and passing `ssl_context=` to `init_poolmanager` is the standard, long-documented way to customise TLS in requests (ciphers, minimum versions, OS truststores, mTLS contexts). After this PR, every `verify=True` request silently replaces that context with the certifi-only global. A base/head probe using the same subclass printed `honored: True` at the merge-base and `honored: False` at head. This is a layering defect: a module-level helper that only knows `(request, verify, cert)` is making a decision that belongs to the adapter, which knows its own pool manager configuration. The fix is to let `_get_connection` pass the pool manager (or its `connection_pool_kw`) into the kwargs builder and inject the default context only when the adapter hasn't supplied one. See `01_shared_ssl_context_ownership.md`.

### 3. The meaning of `verify` is now decided twice, with copy-pasted `isdir` dispatch and disagreeing type predicates (CONFIRMED)

`_urllib3_request_context` (`src/requests/adapters.py:91-101`) now forks `verify` into `False` / `True` / `isinstance(str)` and dispatches `ca_certs` vs `ca_cert_dir` on `os.path.isdir`. `cert_verify` (`src/requests/adapters.py:297-317`), which `send()` runs straight afterwards on the same pool, forks it again into `True` / `is not True` and repeats the same `isdir` dispatch. For `str` paths the second branch rewrites values the pool already has, and the author's review reply acknowledges it is redundant. For truthy non-`str` values such as `pathlib.Path`, only the second layer configures trust. So the new comment "`verify` must be a str with a path then" is false, and the pool is keyed without `ca_certs` and mutated afterwards. For `verify=True`, `cert_verify` now deliberately does nothing and depends, through a four-line comment, on the first function having injected the global context. Any pool that did not get that context (finding 2 is one route) now silently verifies against urllib3's OS default store instead of certifi. This is the textbook "refactor that moves complexity around without deleting it". The code-judo move is a single `_trust_kwargs(verify, client_cert, poolmanager)` decision table that owns the True/False/path dispatch, `os.fspath`, the existence check and the `isdir` split. `_urllib3_request_context` consumes it, and `cert_verify` loses its own fork. A worked version is in `02_trust_policy_layering.md`.

### 4. Building the context at import time moves the cost and a new failure mode into `import requests` (CONFIRMED except the no-ssl case, which is PLAUSIBLE)

`src/requests/adapters.py:75-78` runs `create_urllib3_context()`, zip extraction and `load_verify_locations()` when the module is imported. Measured with `-X importtime`, the self time of `requests.adapters` went from about 0.2 ms to about 4.4 ms, and every process pays it, including those that never verify TLS. With the certifi path made unavailable, the merge-base imported fine and raised the actionable `OSError("Could not find a suitable TLS CA certificate bundle, invalid path: …")` at request time. The head fails the import itself with a bare `FileNotFoundError`, because the PR also deleted that error branch from `cert_verify`. Reading urllib3's `create_urllib3_context`, it raises `TypeError` when Python has no `ssl` module, so `import requests` would now fail on such interpreters as well (not run here). The remedy is a lazily built, cached accessor (`_default_ssl_context()` or `functools.lru_cache(maxsize=1)`). It keeps the performance benefit, since the first request pays once, and removes all three effects. See `03_import_time_side_effects_and_tests.md`.

### 5. A change to TLS trust selection landed with no regression tests, although the needed seam is easy to reach (CONFIRMED)

The author said they could not find a way to observe which context a request uses. `adapter._get_connection(prepared_request, verify, cert=...)` returns the pool, and `pool.conn_kw["ssl_context"]` is the context identity; that is how every probe in this review worked, with no network. Four cheap tests would have caught findings 1 and 2 before merge: `verify=True` gets the shared context; `verify=True` with `cert=` does not; an adapter-supplied `ssl_context` survives; `verify=False` and path-valued `verify` do not get the shared context. See `03_import_time_side_effects_and_tests.md`.

## Question

Nate Prewitt's approval asked whether the context should be per-adapter rather than global. Per-adapter ownership would limit the blast radius of finding 1 to one Session, but it would not fix it: a Session that sends `cert=` to host A and plain requests to host B would still leak within itself. Do the maintainers want the default context to stay process-global and guarded by the "only share when nothing will write to it" rule, or should it be adapter-owned *and* guarded?

## Proposed remediation sequence

1. Replace the eager module global with a lazily built, cached `_default_ssl_context()`, and restore the missing-bundle `OSError` (finding 4).
2. Pass the adapter's pool manager into the kwargs builder. Inject the default context only when `verify is True`, `client_cert is None`, and the pool manager has no `ssl_context` of its own. Everything else returns to merge-base behaviour (findings 1 and 2).
3. Collapse the two `verify` forks into one `_trust_kwargs` decision table used by `_urllib3_request_context`, and cut `cert_verify`'s trust half down to what subclass compatibility requires (finding 3).
4. Add the four `pool.conn_kw["ssl_context"]` identity tests (finding 5).
5. Only after that, consider the larger follow-up of caching contexts per `(ca_location, cert_file, key_file)` so that path-valued `verify` and mTLS also skip repeated loads. Keep it out of this PR, because it requires `cert_verify` to stop writing `conn.cert_file`.

## Detail files

- `01_shared_ssl_context_ownership.md`: urllib3 write paths, probe output, the override probe, worked ownership proposal, test list.
- `02_trust_policy_layering.md`: side-by-side of the two `verify` forks, predicate mismatch, worked `_trust_kwargs` proposal.
- `03_import_time_side_effects_and_tests.md`: import-time measurements, missing-bundle base/head probe, no-ssl analysis, test run output.
