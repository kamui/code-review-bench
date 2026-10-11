# 01 — Ownership of the shared `SSLContext`

Scope: `src/requests/adapters.py:75-78` (the module-level `_preloaded_ssl_context`) and
`src/requests/adapters.py:91-109` (the `verify is True` arm of `_urllib3_request_context`
and the client-certificate kwargs that follow it). Carries the evidence for summary
findings F1, F2 and F5.

## What the change does

At import, the module builds one `ssl.SSLContext` and loads the default CA bundle into it.
For every request with `verify is True`, `_urllib3_request_context` puts that single object
into `pool_kwargs["ssl_context"]`. `_get_connection` then passes `pool_kwargs` to
`PoolManager.connection_from_host(..., pool_kwargs=pool_kwargs)`, where urllib3 merges it
*over* the manager's own `connection_pool_kw`.

The premise in the PR body is that the context is a read-only bag of trust anchors. That
premise is wrong, and the review thread already contains the warning: "It is thread safe as
long as you don't reconfigure it once it is used by a connection ... changing ciphers,
verification settings, or mTLS certs can lead to surprising behavior." The follow-up
(renaming the global with a leading underscore) only discourages *users* from reconfiguring
the context. It does nothing about urllib3 reconfiguring it on requests' behalf, which is
what happens on every connection.

## urllib3 writes to whatever context it is handed

Read from the installed urllib3 2.8.0 (`clone-cache/venv/lib/python3.13/site-packages/urllib3`):

- `connection.py:1040-1042` — when a context is supplied, `context = ssl_context` and then
  `context.verify_mode = resolve_cert_reqs(cert_reqs)`; a write on every connection.
- `connection.py:1045-1056` — `context.check_hostname = False` whenever the pool has
  `assert_fingerprint`, `assert_hostname` (including `assert_hostname is False`), or pyOpenSSL
  is injected. On a shared object that flag never comes back.
- `util/ssl_.py:429-433` — `if certfile: context.load_cert_chain(certfile, keyfile)`; the
  client certificate and private key are loaded into the supplied context.
- `util/ssl_.py:435` — `context.set_alpn_protocols(ALPN_PROTOCOLS)`.

So a context that is passed as `ssl_context` is, by urllib3's contract, owned by the pool it
is passed to. The diff hands the same object to every verified pool in the process.

## F1 — a client certificate bleeds into every other verified request (verified)

`_urllib3_request_context` adds `ssl_context` at line 95 and then, independently, adds
`cert_file` / `key_file` at lines 102-109. Both end up on the same pool, so
`load_cert_chain` runs against the process-global context. From then on every pool that
receives that context — any session, any adapter, any host — holds a context with that
client certificate and key installed, and OpenSSL presents it to any server that sends a
`CertificateRequest`.

Probe: `clone-work/scratch/test_scratch_probe.py::test_probe_A_client_cert_bleeds_through_shared_context`.
A local TLS server with `verify_mode = CERT_OPTIONAL` records `getpeercert()` per
connection; certificates come from a throwaway `trustme` CA; each request uses a brand-new
`Session` with `trust_env = False`. The scratch CA is added to the real
`requests.adapters._preloaded_ssl_context` so the real shared object is exercised.

```
PROBE_A control (verify=<path>): client cert seen per request: [True, False]
PROBE_A verify=True: client cert seen per request: [False, True, True]
PROBE_A third request peercert subject: (('email', 'client@example.test'),)
```

The control row is the pre-change shape (no shared context): the request without `cert=`
presents nothing. The `verify=True` row is the head: request 1 presents nothing, request 2
passes `cert=`, request 3 is a fresh session with no `cert=` and still authenticates as
`client@example.test`.

Two further consequences follow from the same mechanism by reading, not by a probe: the
private key stays loaded in the global context after the session that supplied it is
closed, and two threads using different client certificates race between
`load_cert_chain` and `wrap_socket` on the same object, so a connection can go out with
the other caller's identity.

At the merge-base none of this exists: no `ssl_context` is passed, so urllib3 takes the
`ssl_context is None` branch (`connection.py:1031-1038`) and builds a fresh context per
connection.

## F2 — the shared context overrides TLS settings supplied by adapter subclasses (verified)

Passing `ssl_context=`, `ssl_version=`, or cipher settings through `init_poolmanager` is the
supported way to customise TLS in requests; `docs/user/advanced.rst:1023-1033` documents
exactly that with `Ssl3HttpAdapter`. Per-request `pool_kwargs` override manager-level
kwargs, so at the head:

- an adapter-supplied `ssl_context` is replaced by `_preloaded_ssl_context` whenever
  `verify is True` (the default);
- an adapter-supplied `ssl_version` (the documented example) is ignored, because urllib3
  only consults `ssl_version` in the `ssl_context is None` branch
  (`connection.py:1031-1038`).

Probe: `test_probe_B_adapter_supplied_tls_settings_are_overridden`.

```
PROBE_B custom ssl_context survives: False | is preloaded: True
PROBE_B docs adapter: ssl_version kw = None | ssl_context is preloaded: True
PROBE_B custom ssl_context survives for verify=<path>: True
```

The last line matters for diagnosability: the same adapter keeps its context when `verify`
is a path (for instance when `REQUESTS_CA_BUNDLE` is set) and loses it when `verify` is
`True`. An adapter author sees their TLS policy silently applied or not depending on an
environment variable. (`ssl_version kw = None` is only because urllib3 stores that argument
as a pool attribute rather than in `conn_kw`; the point of the line is the second column.)

## F5 — the context is built as an import side effect (verified)

Lines 75-78 run at `import requests`. Consequences, each of which the merge-base did not have:

1. **Import can now fail.** If the default bundle is missing or unreadable,
   `load_verify_locations` raises at import. Previously that surfaced as an `OSError` on the
   first `verify=True` request and left `verify=False`, `verify=<path>` and plain HTTP
   usable. Probe `test_scratch_probe2.py::test_probe_F_missing_default_bundle_breaks_import`:
   `PROBE_F re-executing requests.adapters with a missing default bundle -> FileNotFoundError: [Errno 2] No such file or directory`.
2. **Interpreters without `ssl`.** `src/requests/__init__.py:120-123` deliberately tolerates
   `import ssl` failing. `create_urllib3_context` raises
   `TypeError("Can't create an SSLContext object without an ssl module")`
   (`urllib3/util/ssl_.py:223-224`) in that case, so the module body would abort the import.
   Read from source; not executed (no ssl-less interpreter available here).
3. **The bundle path is frozen.** The merge-base evaluated
   `extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)` per request inside `cert_verify`; the head
   reads it once. Probe `test_probe_G_bundle_path_frozen_at_import`: after
   `monkeypatch.setattr(adapters, "DEFAULT_CA_BUNDLE_PATH", ...)` the pool kwargs are still
   `['cert_reqs', 'ssl_context']` with the unchanged 121-certificate store.
4. **Filesystem writes at import.** For zipapp-style installs `extract_zipped_paths` copies
   the bundle into the temp directory (`src/requests/utils.py:296-302`); that now happens on
   import rather than on first verified request.
5. **Everyone pays, including callers who never verify with the default bundle.** Measured
   cost on this machine is small — `PROBE_E` 2.6-2.9 ms warm, `PROBE_H` 4.4-4.7 ms in a fresh
   interpreter (next to ~36 ms for importing urllib3 and certifi), OpenSSL 3.5.8. The PR
   body's own profile (0.68 s over 30 calls on OpenSSL 3.0.2) implies it is several times
   larger on the versions the PR is aimed at. The cost is not the main objection; items 1-4
   are.

## Worked proposal

The structural move is to stop treating "which context" as a side effect of `verify is True`
and make it one named decision whose preconditions are visible. Nothing below changes
behaviour for requests that are eligible for sharing; it removes sharing from the requests
that need a context of their own.

```python
@functools.lru_cache(maxsize=None)
def _default_ssl_context():
    """Context with the default CA bundle loaded. Built on first use, never at
    import, and only ever handed to pools that will not reconfigure it."""
    context = create_urllib3_context()
    context.load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))
    return context


# pool kwargs that mean "the adapter author configured TLS themselves"
_ADAPTER_TLS_KWARGS = frozenset(
    {"ssl_context", "ssl_version", "ssl_minimum_version", "ssl_maximum_version",
     "assert_hostname", "assert_fingerprint", "ca_certs", "ca_cert_dir", "ca_cert_data"}
)


def _shared_ssl_context(verify, client_cert, manager):
    """Return the process-wide default context, or None when this request needs
    a context of its own."""
    if verify is not True or client_cert is not None:
        return None  # urllib3 would load_cert_chain() into the shared object
    if _ADAPTER_TLS_KWARGS & manager.connection_pool_kw.keys():
        return None  # the adapter's own TLS configuration wins
    return _default_ssl_context()
```

`_get_connection` already knows which manager it is about to use, so it passes it down, and
`_urllib3_request_context` asks the question once. The eligibility rules that today exist
only as accidents (F1, F2) become three readable lines, and the lazy accessor deletes every
import-time item under F5. `assert_hostname` / `assert_fingerprint` are in the set because
of the `check_hostname = False` write described above. See `02_verify_policy_split.md` for
the one further exclusion (HTTPS proxies) and for how `cert_verify` should consume the same
decision instead of re-deriving it.

A test that pins the ownership rule is short and needs no server:

```python
def test_shared_context_not_used_with_client_cert():
    req = requests.Request("GET", "https://example.test/").prepare()
    pool = HTTPAdapter()._get_connection(req, True, cert="client.pem")
    assert pool.conn_kw.get("ssl_context") is None
```

## Verification status

| Claim | Status |
| --- | --- |
| Client certificate presented by a later, unrelated `verify=True` session | Verified by execution (PROBE_A) |
| Key retained after session close; cross-thread identity race | Read from urllib3 source; not executed |
| Adapter `ssl_context` replaced; docs `ssl_version` adapter given the shared context | Verified by execution (PROBE_B) |
| `check_hostname` flipped on the shared object by `assert_hostname` pools | Read from `connection.py:1045-1056`; not executed. urllib3 falls back to its own `_match_hostname` (`connection.py:1098-1119`), so this is shared-state leakage, not a verification bypass |
| Import fails on missing bundle | Verified by execution (PROBE_F, via `importlib.reload`) |
| Import fails without `ssl` | Read from source; not executed |
| Bundle path frozen at import | Verified by execution (PROBE_G) |
| Import-time cost | Measured (PROBE_E, PROBE_H) |

Commands (run from the clone root; scratch files live outside the clone):

```
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=$PWD/src ../clone-cache/venv/bin/python -m pytest -p no:cacheprovider -s -q ../clone-work/scratch/test_scratch_probe.py
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=$PWD/src ../clone-cache/venv/bin/python -m pytest -p no:cacheprovider -s -q ../clone-work/scratch/test_scratch_probe2.py
```

Both runs: all probes executed (5 passed; 3 passed). The probes print observations rather
than assert, so "passed" means "ran to completion"; the quoted `PROBE_*` lines are the
evidence.
