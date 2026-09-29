# Detail: `src/requests/adapters.py` TLS context sharing

Scope: the whole diff (`git diff main...review-head`), one file. File size at head: 616 lines (`wc -l`), well below the 1000-line threshold, so no decomposition concern from size.

## Changes in the diff

1. New imports `create_urllib3_context` and a module-level `_preloaded_ssl_context` created and populated at import (`adapters.py:75-78`).
2. `_urllib3_request_context()` now has `elif verify is True:` adding `pool_kwargs["ssl_context"]`, and a string `verify` is split between `ca_certs` and `ca_cert_dir` by `os.path.isdir` (`adapters.py:91-100`).
3. `cert_verify()` no longer resolves a default bundle for `verify=True` and only sets `ca_certs`/`ca_cert_dir` for string values (`adapters.py:297-317`).

## Evidence for Finding 1 (adapter context overridden)

Command, run from the clone root with the provided virtualenv (no network, scratch file outside the clone):

    PYTHONPATH=src <venv>/bin/python probe.py

The probe subclasses `HTTPAdapter`, sets `ssl_context=<custom>` in `init_poolmanager()`, then calls `_get_connection()` for three `verify` values and checks `conn.conn_kw["ssl_context"] is custom`. Output at head:

    True   custom ctx used: False
    False  custom ctx used: True
    /etc/ssl/certs/ca-certificates.crt  custom ctx used: True

Mechanism: `PoolManager._merge_pool_kwargs` copies `connection_pool_kw` and then applies the per-request overrides, so the request-level `ssl_context` replaces the manager-level one. Status: verified by execution.

## Evidence for Finding 2 (shared context mutated)

urllib3 2.8.0 reading:

- `util/ssl_.py` `ssl_wrap_socket`: `if certfile: context.load_cert_chain(certfile, keyfile)` runs on the supplied context. The `load_verify_locations` branch is skipped only when no `ca_certs` are given, which is why the PR is faster, but the client certificate branch is not guarded.
- `connection.py` `_ssl_wrap_socket_and_match_hostname`: `context.verify_mode = resolve_cert_reqs(cert_reqs)` and, when `assert_hostname`/`assert_fingerprint` is set, `context.check_hostname = False`.

Consequence: any connection built with the shared context and a client cert or hostname override reconfigures the process-wide object. Pool keys separate the pools, but the context object is shared by all of them. Status: verified by reading; a live handshake showing a leaked client certificate was not run.

## Evidence for Finding 3 (import-time work)

`python -X importtime -c "import requests.adapters"` reports `requests` at about 66 ms cumulative, which includes everything requests imports; the share due to the new context was not isolated, so no claim is made about its size beyond the PR's own profile (about 0.68 s of `load_verify_locations` across 30 concurrent calls, i.e. per-call cost is real). The behavioral points are structural: the code runs at import, the default-bundle existence check was removed from `cert_verify()`, and `DEFAULT_CA_BUNDLE_PATH` is captured once. Status: verified by reading.

## Worked code-judo proposal

Goal: keep one `load_verify_locations()` per process without a global mutable and without a second dispatch site.

    @functools.lru_cache(maxsize=None)
    def _default_ssl_context():
        ctx = create_urllib3_context()
        ctx.load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))
        return ctx

    class HTTPAdapter:
        def init_poolmanager(self, connections, maxsize, block=DEFAULT_POOLBLOCK, **pool_kwargs):
            pool_kwargs.setdefault("ssl_context", _default_ssl_context())
            ...

- A subclass that passes its own `ssl_context` keeps it (Finding 1 gone).
- `_urllib3_request_context()` returns to only translating `verify` strings into `ca_certs`/`ca_cert_dir` and `cert` into `cert_file`/`key_file`; the `verify is True` branch disappears.
- Errors from a missing bundle surface on first use (Finding 3).
- For Finding 2, when `cert` or a hostname override is present, either build a fresh context for that pool or copy trust roots into a per-pool context; the cheap part of context creation is `create_urllib3_context()`, the expensive part is the CA load, so sharing the loaded roots is the goal, not sharing the object.

`cert_verify()` then only validates paths and existence and sets `cert_reqs`; its duplicated `isdir` block can be deleted because `pool_kwargs` already carries `ca_certs`/`ca_cert_dir`.

## Tests to add (Finding 5)

- `_get_connection()` twice with `verify=True` returns pools whose `conn_kw["ssl_context"]` is the same object.
- A subclass-provided `ssl_context` is retained for `verify=True`.
- `verify=<dir>` yields `ca_cert_dir`, `verify=<file>` yields `ca_certs`.
- A `cert=` request does not change the context used by a later request without `cert`.

Known unrelated failures: two ssl/verify tests fail at the merge-base as well as at head on this machine (per the execution policy); none was run here.
