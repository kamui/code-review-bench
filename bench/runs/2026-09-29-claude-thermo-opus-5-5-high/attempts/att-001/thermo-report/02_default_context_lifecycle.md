# 02 — Lifecycle of the module-global default context (`_preloaded_ssl_context`)

Scope: `src/requests/adapters.py:75-78` and its single use at adapters.py:94-95.
Findings covered here: F4 and F5 from `summary.md`.

```python
_preloaded_ssl_context = create_urllib3_context()
_preloaded_ssl_context.load_verify_locations(
    extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
)
```

---

## F4 — Eager import-time construction moves filesystem work and failure into `import requests` and deletes the actionable error for a missing default bundle (lifecycle / boundary)

**Evidence.**

- The context is built as an import side effect of `requests.adapters`, which `requests/__init__.py`
  imports unconditionally. `extract_zipped_paths()` may extract `cacert.pem` out of a zip archive
  into a temp directory, and `load_verify_locations()` parses the whole bundle — both now run for
  every process that imports requests, including ones that never make an HTTPS request or always
  pass `verify=<path>`/`verify=False`.
- The PR removed the only guard for the default bundle. On `main`, `cert_verify` did:
  ```python
  if not cert_loc:
      cert_loc = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
  if not cert_loc or not os.path.exists(cert_loc):
      raise OSError("Could not find a suitable TLS CA certificate bundle, invalid path: ...")
  ```
  On head that check exists only inside `if verify is not True:`; the default path is never
  validated before `load_verify_locations` is called on it.
- Probe P4 (`03_verification.md`) simulates a missing certifi bundle (`certifi.where` → nonexistent
  path) before import:
  - base: `import requests` succeeds; the request fails with
    `OSError: Could not find a suitable TLS CA certificate bundle, invalid path: /nonexistent/cacert.pem`.
  - head: `import requests` itself fails with a bare `FileNotFoundError: [Errno 2] No such file or directory`
    — no path in the message, raised from module import, not from a request.
- Cost is modest on this machine (one `load_verify_locations` ≈ 4-5 ms; `import requests.adapters`
  ≈ 57 ms base vs ≈ 67 ms head, single cold runs, noisy) — so this finding is about lifecycle and
  error boundaries, not import speed.

**Why it matters.** A trust store is request-time configuration. Building it at import time couples
module import to the presence and integrity of a data file, makes the failure non-local (the
traceback points at `adapters.py:76` during import of an unrelated module), and loses the
maintained, user-facing error message. It also freezes the choice of bundle at import: anything that
adjusts `DEFAULT_CA_BUNDLE_PATH`/certifi before first use but after import is silently ignored.

**Code-judo proposal.** Replace the eager global with a lazily-initialised, cached accessor that
owns the existence check. It is the same number of lines, removes the import-time side effect, and
restores the error message in one canonical place:

```python
@functools.lru_cache(maxsize=None)
def _default_ssl_context():
    """Process-wide context with the default CA bundle loaded once.

    Never reconfigure this object: it is shared across adapters and threads.
    Only hand it to pools that use cert_reqs=CERT_REQUIRED.
    """
    ca_bundle = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
    if not ca_bundle or not os.path.exists(ca_bundle):
        raise OSError(
            "Could not find a suitable TLS CA certificate bundle, "
            f"invalid path: {ca_bundle}"
        )
    context = create_urllib3_context()
    context.load_verify_locations(ca_bundle)
    return context
```

`lru_cache` on a zero-argument function is thread-safe for the purpose here (at worst two threads
build it once each on a cold start; one wins). The underscore name keeps the "private by
convention" property the maintainers asked for, and the docstring records the invariant from F5
where the object is defined rather than in a comment inside `cert_verify`.

**Verification status.** Import-time failure and message loss verified by probe P4 on head and
base. Timing measured once per tree (not a benchmark). Lazy accessor not applied (read-only review).

---

## F5 — A process-global mutable object is handed to urllib3, which writes to it on every connect; the safety invariant is implicit (hidden coupling)

**Evidence.** urllib3 2.8.0 `_ssl_wrap_socket_and_match_hostname` (`connection.py:1031-1056`) does
not copy a supplied context; it mutates it:

```python
else:
    context = ssl_context
context.verify_mode = resolve_cert_reqs(cert_reqs)          # every connect
if assert_fingerprint or assert_hostname or assert_hostname is False or ...:
    context.check_hostname = False                           # sticky
```

With the PR, one `SSLContext` object is shared by every `HTTPAdapter` in the process for every
`verify=True` request, across threads. The PR discussion (quoting a CPython core developer) sets the
safety condition explicitly: sharing is fine "as long as you don't reconfigure it once it is used by
a connection". The code does reconfigure it on each connect; it is only harmless today because:

1. the context is injected solely on the `verify is True` branch, which always pairs it with
   `cert_reqs="CERT_REQUIRED"`, so the `verify_mode` write is idempotent; and
2. nothing in requests itself passes `assert_hostname`/`assert_fingerprint`.

Neither condition is written down or enforced. A subclass that adds `assert_fingerprint=...` or
`assert_hostname=False` through `init_poolmanager` flips `check_hostname=False` on the shared
context for the rest of the process, for every adapter (urllib3 then falls back to its own
`_match_hostname` for other pools, so this is not a verification bypass today, but it is
cross-adapter action-at-a-distance). And any future refactor that moves the default into
adapter-wide kwargs (the tempting "fix" for F2) would pair it with `CERT_NONE` and globally disable
verification for concurrent `verify=True` requests.

**Remedy.** Keep the context injection structurally tied to `CERT_REQUIRED` (as in the F1 proposal,
where it can only be reached inside the `verify is not False` arm), record the invariant in the
accessor's docstring (F4 proposal), and add a one-line test that the default context's
`verify_mode` is still `CERT_REQUIRED` and `check_hostname` still `True` after a
`verify=False` request on the same adapter. If maintainers want belt-and-braces, the default
could be skipped whenever the merged pool kwargs contain `assert_hostname`/`assert_fingerprint`,
but documenting and testing the invariant is the proportionate step.

**Verification status.** urllib3 mutation verified by reading the installed urllib3 2.8.0 source.
The cross-adapter `check_hostname` flip was reasoned from that source, not executed (it requires a
live TLS handshake). Classified PLAUSIBLE as a maintainability hazard, not a present-day bug.
