# 03 - Verification log and test coverage

Carries the commands and results behind every "verified by execution" statement in the other
files, and the evidence for summary finding F5.

## Environment

- Clone at `review-head` = `4089f3dc`, merge-base `main` = `8dd3b26b`. The clone was not
  modified; `git status --short` was empty after each run.
- Provided virtualenv: CPython 3.13.15, OpenSSL 3.5.8, urllib3 2.8.0, pytest 6.2.5, trustme.
- Only the head was executed. Statements about merge-base behaviour come from reading the
  diff, and each probe that needs a baseline carries an in-process control that follows the
  code path the merge-base used.
- No network. The probes talk only to TLS servers they start on 127.0.0.1.

## Static measurements

```
git diff main...review-head --stat
 src/requests/adapters.py | 46 ++++++++++++++++++++++++++++------------------
 1 file changed, 28 insertions(+), 18 deletions(-)

wc -l src/requests/adapters.py                    -> 616
git show main:src/requests/adapters.py | wc -l    -> 606
git diff main...review-head --stat -- tests docs HISTORY.md   -> (empty)
```

Search for the new names across `*.py`, `*.rst`, `*.md` in the clone: `_preloaded_ssl_context`
appears only at `src/requests/adapters.py:75`, `:76` and `:95`. `ssl_context` appears in
`tests/` only in `tests/testserver/server.py` (the server side). No test references
`_urllib3_request_context`, `_get_connection` or `cert_verify`.

## Probe suite

File: `<work>/scratch/test_probe.py` (outside the clone). Every test asserts the behaviour
observed at the head, so a pass means the described behaviour is confirmed.

Command, run from the clone root:

```
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=<clone>/src <cache>/venv/bin/python -m pytest \
    -p no:cacheprovider -s -v -W ignore -o addopts="" <work>/scratch/test_probe.py
```

Result: `6 passed in 1.67s`.

| Probe | What it establishes | Supports |
| --- | --- | --- |
| `test_1_client_cert_leaks_into_other_sessions` | After one request with `cert=`, a new `Session` and bare `requests.get` present that certificate to a different server | F1 |
| `test_2_custom_ssl_context_is_replaced_when_verify_true` | An adapter's own `ssl_context` is kept for `verify=<path>` and replaced by `_preloaded_ssl_context` for `verify=True` | F2 |
| `test_3_tls_version_pin_is_ignored_when_verify_true` | `ssl_minimum_version=TLSv1_3` adapter: `SSLError` against a TLS 1.2 server with `verify=<path>`, success at `TLSv1.2` with `verify=True` | F2 |
| `test_4_assert_hostname_adapter_mutates_shared_context` | `_preloaded_ssl_context.check_hostname` goes `True` -> `False` after one request through an `assert_hostname` adapter | F1 |
| `test_5_legacy_get_connection_plus_cert_verify_loses_certifi` | `get_connection()` + `cert_verify(..., True, None)` leaves `CERT_REQUIRED` with no CA file, no CA dir, no context | F3 |
| `test_6_import_time_cost_and_failure` | Module body raises `FileNotFoundError` when the default bundle path is missing; context build measured at 2.7-3.4 ms | F4 |

Captured output:

```
test_1  before:    {'client_cert_presented': False, 'tls': 'TLSv1.3'}
        with_cert: {'client_cert_presented': True,  'tls': 'TLSv1.3'}
        after:     {'client_cert_presented': True,  'tls': 'TLSv1.3'}
        plain api: {'client_cert_presented': True,  'tls': 'TLSv1.3'}
test_3  negotiated with a TLSv1.3-minimum adapter: {'client_cert_presented': True, 'tls': 'TLSv1.2'}
test_5  cert_reqs: CERT_REQUIRED ca_certs: None ca_cert_dir: None ssl_context: None
test_6  module-level context build, ms: [3.3, 3.4, 2.9, 2.8, 2.7]
```

Note the `client_cert_presented: True` in the `test_3` line: that session never passed a
certificate either. It is the leak from `test_1` still in effect two tests later.

Two things about the probe setup a reader should know. The probes add a throwaway trustme CA
to `_preloaded_ssl_context` with `load_verify_locations` so that `verify=True` can reach a
local server; that is additive trust-store setup and does not touch the mechanisms under test.
And `test_6` exercises the import failure by re-executing the module body with
`importlib.reload` under a patched `requests.utils.DEFAULT_CA_BUNDLE_PATH`, not by starting a
fresh interpreter.

## Existing suite

```
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=<clone>/src <cache>/venv/bin/python -m pytest \
    -p no:cacheprovider -q -W ignore tests/test_adapters.py tests/test_requests.py \
    -k "verify or ssl or cert or tls or adapter or pool"
```

Result: `3 failed, 21 passed, 306 deselected in 2.77s`.

- `TestRequests::test_pyopenssl_redirect` and
  `TestPreparingURLs::test_different_connection_pool_for_tls_settings_verify_bundle_unexpired_cert`
  are the two failures the execution policy records as failing at the merge-base as well
  (fixture certificate vs. installed OpenSSL). Not attributed to the change.
- `TestPreparingURLs::test_different_connection_pool_for_mtls_settings` also failed. The final
  traceback in the run ends in `SSLV3_ALERT_CERTIFICATE_EXPIRED`, i.e. the server rejected the
  fixture client certificate as expired. The test's first request is
  `s.get(url, verify=False, cert=cert)` (`tests/test_requests.py:2959`), a path that does not
  use the shared context, so I do not attribute this to the change either. I did not run the
  merge-base to confirm, and this failure is not in the policy's list of known ones, so treat
  the attribution as my reading rather than a measured fact.

## F5 - no tests accompany the change

The diff contains no test and no changelog entry. The author raised this in the review thread
and asked for pointers because they "couldn't find a way to access such low-level information
about a request in the exposed classes". The information is reachable without a network:
`adapter._get_connection(prepared_request, verify)` returns the pool, and `pool.conn_kw`
exposes `ssl_context`, `ca_certs`, `ca_cert_dir` and `cert_reqs`. Probes 2 and 5 above are
about ten lines each and use nothing else.

Tests the change should carry, each of which would have failed against the head:

1. `verify=True` with `cert=` does not load the client certificate into a context any other
   pool can see (assert context identity differs, or drive two local servers as probe 1 does;
   the suite already has `TLSServer(mutual_tls=True)` in `tests/testserver/server.py`).
2. An adapter that passes `ssl_context=` to `init_poolmanager` gets that context back from
   `_get_connection` for `verify=True`.
3. An adapter that passes `ssl_minimum_version=` / `ssl_version=` keeps that setting for
   `verify=True`.
4. `get_connection()` followed by `cert_verify(conn, url, True, None)` leaves the default
   bundle configured on the pool.
5. `verify=<directory>` produces `ca_cert_dir`, `verify=<file>` produces `ca_certs`, in both
   the pool keywords and on the pool (the branch the PR duplicated has no direct test).
6. Importing `requests.adapters` does not build a context (assert the cache is empty after
   import once the context is lazy).

Relatedly, `tests/test_requests.py:2962` (`s.get(url, cert=cert)` under the default
`verify=True`) currently writes the mTLS fixture certificate into the process-wide context and
leaves it there for every later test in the run. Once the shared context is no longer used for
requests that carry a client certificate, that cross-test contamination goes away on its own.
