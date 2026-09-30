# Review blind-9e3449

### Item 1
Location: src/requests/adapters.py:91-317
Claim: In `src/requests/adapters.py`, `_urllib3_request_context` (lines 91–100) now decides part of
the trust policy: `verify is True` means use the global preloaded context, and a string means
`isdir` → `ca_cert_dir`, else `ca_certs`. `cert_verify` (lines 297–317) still runs the
existence check and then repeats the same `isdir` dispatch on the pool. To find out which trust
store a request uses, a reader has to follow both functions plus the comment at lines 300–303,
which states an invariant nothing enforces ("the connection will be using a context with the
default certificates already loaded"). That comment also calls `verify` "a boolean" in a branch
`False` never reaches, and it pairs with an unchecked "must be a str then" assumption. The split
also limits the speedup to the literal `verify=True` case. Custom bundles still set `ca_certs`,
so urllib3 reloads them on every connection. The same goes for every `REQUESTS_CA_BUNDLE` /
`CURL_CA_BUNDLE` user, because `Session.merge_environment_settings` turns `True` into a path
string. The code-judo move: one helper maps `verify` to pool kwargs, and every verifying case
gets an `lru_cache`'d context keyed by CA path. The helper does the existence check and the
file/dir dispatch once. The `verify is True` branch goes away because the default bundle is
just another path, and `cert_verify` keeps only client-cert handling plus a legacy fallback.
Full evidence and the worked proposal are in `01_adapters_tls_policy.md` § Finding 1.
Consequence: —
Fix: —

### Item 2
Location: src/requests/adapters.py:94-95
Claim: `_urllib3_request_context` lines 94–95 always write
`pool_kwargs["ssl_context"] = _preloaded_ssl_context`. urllib3 merges per-request pool kwargs
over the manager's `connection_pool_kw`. So an `HTTPAdapter` subclass that overrides the
documented `init_poolmanager` hook to pass its own `ssl_context` (custom ciphers, truststore,
and so on) loses that context whenever `verify=True`, which is the default. There is no error.
A probe confirmed it: at head, the pool for `verify=True` carries the preloaded context and not
the subclass's. At the merge-base, all three verify modes keep the custom context. This is an
optimisation for the default configuration placed in a path that every configuration shares.
The fix is to make the default context a fallback: inject it only when the manager has no
`ssl_context`, by passing the manager's kwargs into the helper from Finding 1. Add a regression
test asserting `conn.conn_kw["ssl_context"] is custom`. See `01_adapters_tls_policy.md` §
Finding 2.
Consequence: —
Fix: —

### Item 3
Location: src/requests/adapters.py:75-78
Claim: Lines 75–78 build an `SSLContext` and load the certifi bundle during `import requests`, and the
PR deletes the old `os.path.exists` guard for the default path. Probes against head and the
merge-base confirmed four consequences. First, with `ssl` unavailable, base imports fine and
head fails with `TypeError: Can't create an SSLContext object without an ssl module`. Second,
when the bundle is missing, base raises
`OSError: Could not find a suitable TLS CA certificate bundle, invalid path: ...` at request
time, while head raises a bare `FileNotFoundError` from `import requests`. That hurts exactly
the zipapp/freezer users the review was worried about, even programs that never make an HTTPS
request. Third, reassigning `requests.adapters.DEFAULT_CA_BUNDLE_PATH` after import is no longer
honoured. Fourth, `extract_zipped_paths` now writes to a temp directory at import. The remedy
is a lazy, memoised accessor that reads `DEFAULT_CA_BUNDLE_PATH` at call time and keeps the
original `OSError`. That keeps the full performance win, since the bundle still loads once per
process, and the helper from Finding 1 already has this shape. See `01_adapters_tls_policy.md`
§ Finding 3.
Consequence: —
Fix: —

### Item 4
Location: tests/test_requests.py:2833
Claim: The PR adds no tests. The author said in review that the context was unreachable, but it is
exposed as `adapter._get_connection(prepared, verify).conn_kw["ssl_context"]`, and the existing
`test_different_connection_pool_for_tls_settings_*` tests (`tests/test_requests.py:2833`
onward) already call `_get_connection`. Add small assertions for three cases: `verify=True`
uses the shared context, a custom bundle does not touch it, and a subclass context wins over
the default. Add an import-side-effect test once Finding 3 is fixed. Finding 2 would have been
caught by the third assertion. See `01_adapters_tls_policy.md` § Finding 4.
Consequence: —
Fix: —
