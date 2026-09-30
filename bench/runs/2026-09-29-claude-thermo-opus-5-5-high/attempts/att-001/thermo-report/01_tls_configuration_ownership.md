# 01 — Who owns TLS trust configuration? (`_urllib3_request_context` vs `cert_verify`)

Scope: `src/requests/adapters.py` at `review-head` (4089f3dc), compared with `main` (8dd3b26b).
Findings covered here: F1, F2, F3, F6 from `summary.md`.

## Background: the two paths that configure a pool

Since the merge-base, `HTTPAdapter.send()` does two things with trust configuration for every request:

1. `_get_connection()` → `_urllib3_request_context()` (adapters.py:81-115) builds `pool_kwargs`
   (`cert_reqs`, `ca_certs`/`ca_cert_dir`, and now `ssl_context`) and hands them to
   `PoolManager.connection_from_host(..., pool_kwargs=...)`. urllib3 keys pools on those values, so
   each distinct TLS configuration gets its own pool, configured at construction time.
2. `cert_verify()` (adapters.py:285-338) is then called on that same pool and **mutates** it:
   `conn.cert_reqs`, `conn.ca_certs`, `conn.ca_cert_dir`, `conn.cert_file`, `conn.key_file`.

Before this PR the two paths were redundant but agreed (both set `cert_reqs`; `cert_verify`
additionally pointed `ca_certs` at the certifi bundle for `verify=True`). This PR deliberately makes
them disagree for `verify=True`: the pool key now carries `ssl_context=_preloaded_ssl_context`, and
`cert_verify` stops writing the default bundle, because writing `conn.ca_certs` would make urllib3's
`ssl_wrap_socket` call `load_verify_locations()` again on every connect (urllib3 `util/ssl_.py:415`),
which is the cost the PR is trying to remove.

---

## F1 — TLS trust configuration now has two owners with duplicated, diverging dispatch (structural, presumptive blocker)

**Evidence.** After the PR, the `verify` → trust-store dispatch is written twice, in two different
shapes:

```python
# _urllib3_request_context, adapters.py:91-101
cert_reqs = "CERT_REQUIRED"
if verify is False:
    cert_reqs = "CERT_NONE"
elif verify is True:
    pool_kwargs["ssl_context"] = _preloaded_ssl_context
elif isinstance(verify, str):
    if not os.path.isdir(verify):
        pool_kwargs["ca_certs"] = verify
    else:
        pool_kwargs["ca_cert_dir"] = verify
pool_kwargs["cert_reqs"] = cert_reqs
```

```python
# cert_verify, adapters.py:297-321
if url.lower().startswith("https") and verify:
    conn.cert_reqs = "CERT_REQUIRED"
    # (4-line comment about avoiding load_verify_locations)
    if verify is not True:
        # `verify` must be a str with a path then
        cert_loc = verify
        if not os.path.exists(cert_loc):
            raise OSError(...)
        if not os.path.isdir(cert_loc):
            conn.ca_certs = cert_loc
        else:
            conn.ca_cert_dir = cert_loc
else:
    conn.cert_reqs = "CERT_NONE"
    conn.ca_certs = None
    conn.ca_cert_dir = None
```

The PR *added* the `isdir` split to `_urllib3_request_context` (it was only in `cert_verify` before),
so the file/dir decision, the `cert_reqs` decision, and the "what does `verify` mean" decision are
now each made twice per request (two `isdir` syscalls on the `str` path). The two copies already
disagree on type dispatch (`verify is True` / `isinstance(verify, str)` / fall-through vs.
`verify is not True` + an unenforced comment "must be a str with a path then") and on falsy
handling (`verify is False` vs. truthiness). The author says as much in the PR discussion: "I
believe this is now fully redundant with the corresponding logic in `cert_verify()`."

More importantly, the correctness of `cert_verify` for `verify=True` now depends on an invariant
established in a different function: "this pool was built by `_urllib3_request_context` and already
carries `_preloaded_ssl_context`". The explanatory comment inside `cert_verify` is the only place
this coupling is recorded. Anything that reaches `cert_verify` with a pool not built that way gets a
different trust store (see F3, verified).

**Why this is structural, not stylistic.** The PR's optimisation is really "trust configuration is
decided once, when the pool is keyed, and never re-applied per connection". That is a clean idea,
but the diff implements it by carving an exception out of the per-connection mutation path instead
of retiring that path. The result is more concepts for the reader (pool-key config, per-pool
mutation, and a special case where the mutation is intentionally skipped), not fewer.

**Code-judo proposal.** Make `_urllib3_request_context` the single owner of TLS pool configuration and
reduce `cert_verify` to what it can uniquely do: validate user-supplied paths and raise the
documented `OSError`s. Behaviour for `send()` is unchanged because every value `cert_verify` writes
is already present in the pool key.

```python
def _urllib3_request_context(request, verify, client_cert, poolmanager):
    ...
    if verify is False:
        pool_kwargs["cert_reqs"] = "CERT_NONE"
    else:
        pool_kwargs["cert_reqs"] = "CERT_REQUIRED"
        if isinstance(verify, str):
            key = "ca_cert_dir" if os.path.isdir(verify) else "ca_certs"
            pool_kwargs[key] = verify
        elif "ssl_context" not in poolmanager.connection_pool_kw:   # see F2
            pool_kwargs["ssl_context"] = _default_ssl_context()      # see summary F4
    ...

def cert_verify(self, conn, url, verify, cert):
    """Validate TLS file arguments. Pool TLS settings come from the pool key."""
    if url.lower().startswith("https") and isinstance(verify, str) \
            and not os.path.exists(verify):
        raise OSError(f"Could not find a suitable TLS CA certificate bundle, "
                      f"invalid path: {verify}")
    ...existing cert/key existence checks, without assigning conn.cert_file/key_file...
```

What disappears: the `conn.cert_reqs` writes (both branches), the `conn.ca_certs`/`conn.ca_cert_dir`
writes and resets, the second `isdir`, the four-line "we intentionally don't load anything"
comment, and the `# verify must be a str` assumption. `cert_verify` keeps its public signature and
still raises the same errors, so subclass callers of it keep working. If the maintainers are not
ready to retire the mutation for the legacy `get_connection()` path, the compatibility branch in F3
is the one place it should live, clearly labelled as legacy.

**Verification status.** Duplication and divergence: verified by reading the diff
(`git diff main...review-head`). That the pool key already carries every value `cert_verify`
writes on the `send()` path: verified by probe P3 (`03_verification.md`) — the pool returned by
`_get_connection(req, True)` has `ssl_context` set, and `cert_verify` adds nothing to it. The
refactor itself was not applied (review is read-only).

---

## F2 — The per-request default `ssl_context` overrides an adapter's own `ssl_context` (boundary regression, verified)

**Evidence.** `_urllib3_request_context` puts `ssl_context=_preloaded_ssl_context` into the
*per-request override* `pool_kwargs` whenever `verify is True` (adapters.py:94-95). urllib3's
`PoolManager._merge_pool_kwargs` (urllib3 `poolmanager.py:390-410`) copies
`self.connection_pool_kw` and then lets every non-`None` override key win. So the adapter-level
configuration that a subclass passes through the documented extension point
`init_poolmanager(..., **pool_kwargs)` — the canonical way requests users customise TLS
(custom ciphers, legacy renegotiation, pinned CA stores, `truststore` contexts) — is silently
replaced by the module-global context for every `verify=True` request, which is the default.

Probe P1 (`03_verification.md`) builds an `HTTPAdapter` subclass whose `init_poolmanager` injects a
custom `SSLContext`:

| | base (`main`) | head |
| --- | --- | --- |
| `verify=True` pool uses the subclass's context | **True** | **False** (it is `_preloaded_ssl_context`) |
| `verify=False` pool uses the subclass's context | True | True |

So after this PR, customising TLS through `init_poolmanager` works only when the caller *disables*
verification — an inverted and surprising contract.

**Why this is a design problem.** It is a layering leak: a module-level implementation detail
(the preloaded context) is injected at the most specific layer (per-request override), where it
outranks configuration that belongs to a more general but user-owned layer (the adapter's pool
manager). A default should sit *below* user configuration, not above it.

**Remedy.** Only supply the default when nobody above has configured a context. The smallest
correct change is to consult the manager that will serve the request:

```python
def _get_connection(self, request, verify, proxies=None, cert=None):
    proxy = select_proxy(request.url, proxies)
    manager = self.proxy_manager_for(...) if proxy else self.poolmanager
    host_params, pool_kwargs = _urllib3_request_context(request, verify, cert, manager)
    return manager.connection_from_host(**host_params, pool_kwargs=pool_kwargs)
```

with the `"ssl_context" not in poolmanager.connection_pool_kw` guard shown in F1. This also folds the
duplicated proxy/no-proxy `connection_from_host` calls in `_get_connection` into one. Do **not**
"fix" this by moving the default into `init_poolmanager`'s kwargs: that would hand the shared
context to `verify=False` pools too, and urllib3 writes `context.verify_mode = CERT_NONE` onto
whatever context it is given (see summary F5 / `02_default_context_lifecycle.md`).

Add a regression test (see F6) asserting that a subclass-supplied context survives `verify=True`.

**Verification status.** Verified by probe P1 on head and base; urllib3 merge semantics verified by
reading the installed urllib3 2.8.0 source.

---

## F3 — `cert_verify(verify=True)` no longer establishes a trust store on its own; the legacy path silently falls back to the OS store and keeps stale CA state (behavioural consequence of F1, verified)

**Evidence.** `get_connection(url, proxies)` (adapters.py:406-433) is still public and documented
"for use when subclassing"; it builds pools with **no** TLS pool kwargs. Third-party adapters that
override `send()` or call `get_connection()` + `cert_verify()` themselves relied on `cert_verify` to
point the pool at the certifi bundle when `verify=True`. After this PR that branch writes nothing:

- Probe P2: `get_connection("https://example.com/")` then `cert_verify(pool, url, True, None)` —
  base: `pool.ca_certs = …/certifi/cacert.pem`; head: `pool.ca_certs = None`, no `ssl_context`.
  With no `ca_certs`, no `ca_cert_dir`, and no supplied context, urllib3 calls
  `context.load_default_certs()` (urllib3 `connection.py:1061-1068`), i.e. the **OS** trust store
  instead of certifi. That is precisely the "breaks a whole class of users" category the maintainer
  flagged in the review thread (zipapp / pyinstaller users who depend on the bundled certifi file).
- Probe "stale": call `cert_verify(pool, url, "<dir>", None)` then `cert_verify(pool, url, True, None)`
  on the same legacy pool. Head leaves `ca_certs=None, ca_cert_dir=<user dir>` — a subsequent
  `verify=True` request trusts only the previous caller's directory. Base overwrote `ca_certs` with
  certifi (it also left `ca_cert_dir` behind, so the partial-reset smell pre-exists, but the PR
  turns it from "certifi plus leftover dir" into "only the leftover dir").

**Why it matters for design.** This is F1's hidden coupling made concrete: `cert_verify` no longer
describes a complete state transition for its own inputs. The `verify=False` branch resets three
attributes; the `verify=<str>` branch sets one of two and leaves the other; the `verify=True` branch
now sets only `cert_reqs`. Partial updates like this are exactly what makes mutable-pool
configuration hard to reason about.

**Remedy.** Either (preferred) retire the mutation path as in F1 and give the legacy
`get_connection()` path its own explicit, labelled compatibility shim, or keep the mutation but
make it total and conditional on how the pool was built:

```python
if verify is True and conn.conn_kw.get("ssl_context") is None:
    # Legacy pools from get_connection() carry no TLS pool key; keep the pre-2.32 default.
    conn.ca_certs, conn.ca_cert_dir = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH), None
```

and in the `str` branch set the other attribute to `None` so each call is a complete assignment.
Whichever option is chosen, one of the two functions must be the owner; today neither is.

**Verification status.** Verified by probes P2 and "stale" on head and base (`03_verification.md`).
Whether real third-party adapters exercise this path was not measured (no network).

---

## F6 — The optimisation has no tests, although the relevant state is directly inspectable (test coverage)

**Evidence.** The diff touches only `src/requests/adapters.py`; `grep -n "ssl_context\|_get_connection\|conn_kw" tests/*.py`
returns nothing. The author wrote in the review thread that they "couldn't find a way to access such
low-level information about a request". It is reachable without network: the pool returned by
`HTTPAdapter()._get_connection(prepared_request, verify)` exposes `pool.conn_kw["ssl_context"]`,
`pool.ca_certs`, and `pool.ca_cert_dir` — probes P1-P3 do exactly this.

**Remedy.** Add offline unit tests in `tests/test_adapters.py`:

```python
def _pool(adapter, verify):
    req = requests.Request("GET", "https://example.com/").prepare()
    return adapter._get_connection(req, verify)

def test_verify_true_uses_shared_default_context_without_reloading():
    pool = _pool(HTTPAdapter(), True)
    assert pool.conn_kw["ssl_context"] is requests.adapters._default_ssl_context()
    assert pool.ca_certs is None and pool.ca_cert_dir is None

def test_verify_true_respects_adapter_ssl_context():
    ctx = ssl.create_default_context()
    class A(HTTPAdapter):
        def init_poolmanager(self, *a, **kw):
            return super().init_poolmanager(*a, ssl_context=ctx, **kw)
    assert _pool(A(), True).conn_kw["ssl_context"] is ctx

@pytest.mark.parametrize("kind", ["file", "dir"])
def test_verify_path_selects_ca_certs_or_dir(tmp_path, kind): ...
```

These pin the performance invariant (no `ca_certs` on the default pool, so no per-connect
`load_verify_locations`), the F2 regression, and the new `isdir` split.

**Verification status.** Absence of tests verified by grep; the proposed assertions were validated
informally by the probes, but the test code itself was not run.
