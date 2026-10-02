# TLS adapter context ownership — detailed review

## Scope and measurements

The only changed file is `src/requests/adapters.py`. Its line count is 606
at `main` and 616 at `review-head`; the diff is +28/-18. The reviewed head
is `4089f3dc65f783beaa53cc032958ab625440d0ac`, and its tree is
`d1febc2d2f8b1b2877afe593728d74c6f71a81f3`.

Measurements came from:

```sh
git diff main...review-head
git diff main...review-head --numstat
wc -l src/requests/adapters.py
git show main:src/requests/adapters.py | wc -l
git diff --check main...review-head
git status --porcelain=v1
git rev-parse HEAD 'HEAD^{tree}'
```

No file-size blocker exists. The design issue is the relationship among the
global context at lines 75–78, request kwargs at lines 91–109, pool selection
at lines 377–404, and CA/client-certificate mutation at lines 297–338.

## Shared context and client identity

Finding anchor: `src/requests/adapters.py:94–95`, with the global resource
created at lines 75–78 and client credentials passed at lines 102–109.

The helper includes both of these fields for a default-verified client request:

```python
pool_kwargs["ssl_context"] = _preloaded_ssl_context
pool_kwargs["cert_file"] = client_cert
```

This is not an immutable combination of independent settings. In the installed
urllib3's `util/ssl_.py:407–433`, a supplied context is used directly:

```python
context = ssl_context
# ...
if certfile:
    if key_password is None:
        context.load_cert_chain(certfile, keyfile)
    else:
        context.load_cert_chain(certfile, keyfile, key_password)
```

The client identity lives on the context after this call. A subsequent request
without `cert_file` does not clear it. Although PoolManager keys include
`cert_file` and `key_file`, both pools receive the same mutable object.
Session.close() clears that Session's pools, not the global context.

The scratch test `test_client_identity_is_not_inherited` runs one verified
request with a generated client certificate, closes that Session, then creates
another Session and runs a verified request with `cert=None`. The local
server uses CERT_OPTIONAL and trusts the generated client CA. It records
`getpeercert()` for both requests. Both requests succeed on both versions.
At the base, the first peer has a certificate and the second has none.
At the head, the second peer also has the first client's certificate.

This is a confirmed behavior regression and a structural ownership failure.
It does not require concurrent execution or mutation of a private Requests
attribute by a user. A thread race between different cert= values is a further
inferred risk, not a separately executed reproduction. Sending a client
certificate proves possession during TLS; the observed issue is unintended
client authentication, not transmission of the private key.

Changing the cache to one context per adapter would still permit identity
contamination between requests with different credentials or no credentials
within that adapter. A mutex around load_cert_chain() alone would also not
remove persistent identity state. Safe minimal remediation is to leave mTLS
on the previous per-connection context path. More ambitious credential-context
caching would need configuration-complete keys and initialization before use;
it is not necessary for this PR's default-request optimization.

The existing mTLS pool test is insufficient for this invariant. It tests
verify=False success and verify=True failure against untrusted/expired
fixtures, and never follows a successful default-verified client exchange
with a certificate-free request. Its pool count assertions do not inspect
shared context state.

## Adapter customization boundary

Finding anchor: `src/requests/adapters.py:94–95`.

`HTTPAdapter.init_poolmanager()` at lines 221–245 explicitly supports subclass
pool kwargs. `proxy_manager_for()` at lines 247–283 likewise exposes manager
customization. The repository's advanced documentation at
`docs/user/advanced.rst:1008–1033` demonstrates configuring TLS construction
through a PoolManager in an adapter subclass.

The installed PoolManager's `connection_from_host()` merges request kwargs
over `connection_pool_kw`. Its `_merge_pool_kwargs()` at lines 390–410 starts
with a copy of manager defaults, then replaces each matching key with the
request override. Therefore the head's request context necessarily wins over
a manager-supplied SSLContext.

The scratch test constructs an HTTPAdapter subclass that supplies a context
in both manager hooks. It then obtains a pool through the ordinary
`_get_connection(..., verify=True)` path and checks
`pool.conn_kw["ssl_context"] is supplied`. At the base the assertion succeeds
for both direct and HTTP-proxy managers; at the head it fails for both.
This checks actual PoolManager behavior, without a dummy wrapper around
the merge operation and without external networking.

Protocol-construction parameters also require care. Installed urllib3
`connection.py:1030–1040` constructs a context with ssl_version and min/max
settings only when the supplied context is None. A globally supplied context
therefore bypasses that construction. The direct context replacement is
executed evidence; the bypass of construction settings is source evidence,
not a separately performed protocol-negotiation test.

Remediation must occur where the manager and request settings are both known.
A free helper accepting only request, verify, and cert cannot decide whether
an adapter has already chosen its TLS context. Give default-context selection
an adapter-owned boundary and use the manager's existing kwargs rather than
inventing a second registry of settings. A subclass-owned context should
retain the same default-CA behavior as before unless the subclass already
overrides cert_verify(). Do not simply omit ca_certs for every custom context:
before this change Requests loaded its default CA bundle into those contexts.

## Import and initialization boundary

Finding anchor: `src/requests/adapters.py:75–78`.

The new global initializer performs TLS construction and filesystem-dependent
CA loading during module import. Requests imports sessions and adapters as
part of its ordinary package import; this dependency is consequently broader
than a request for verified HTTPS.

The package already accounts for unavailable stdlib ssl in
`src/requests/__init__.py:116–137`: it tries a fallback and catches ImportError.
The base adapter itself does not construct an SSLContext until urllib3 needs
one. The head's unconditional call defeats that deferred boundary.

Two focused tests load each adapter source in a fresh module namespace.
One substitutes a nonexistent path for utils.DEFAULT_CA_BUNDLE_PATH.
The other sets urllib3.util.ssl_.SSLContext to None, simulating an unavailable
backend after fallback. In both cases, the base module loads and an HTTP pool
can be selected. The head fails inside its module initializer:
FileNotFoundError at line 76 for the missing bundle, and TypeError at line 75
for the missing backend.

These are controlled prerequisite simulations, not tests on a separately
installed Python without ssl. They directly exercise the failing source
boundary but do not claim a full package-import trace on that installation.

A lazy accessor preserves reuse without making HTTP, custom-bundle HTTPS,
and verify=False depend on the default CA bundle. It should use
extract_zipped_paths() and preserve the existing invalid-path diagnostic.
Cache only a completely loaded context; a failed load must not publish
half-initialized state. Synchronize initialization to avoid multiple expensive
loads when concurrent requests first use the default path.

## Worked code-judo proposal

The useful reframing is: the shared context is an optimization of one eligible
TLS configuration, not the owner of all boolean-verified requests.

There is no need for a generic TLS-policy hierarchy or a new module merely
to reach this boundary. Keep request host/certificate kwargs in the existing
helper. Choose the direct or proxy manager in _get_connection(), then decide
whether that manager permits the default optimization. Keep ordinary custom
bundle handling as it is until a separate compatible simplification is proven.

A sketch of the selection boundary is:

```python
# In _get_connection(), after selecting the effective manager:
host_params, pool_kwargs = _urllib3_request_context(request, verify, cert)
if (
    host_params["scheme"] == "https"
    and verify is True
    and cert is None
    and uses_plain_requests_tls_defaults(manager.connection_pool_kw)
):
    pool_kwargs["ssl_context"] = _get_preloaded_ssl_context()
return manager.connection_from_host(**host_params, pool_kwargs=pool_kwargs)
```

Here the helper no longer unconditionally inserts the preloaded context.
`uses_plain_requests_tls_defaults` is a design placeholder, not a proposed
pass-through production wrapper. Its contract must exclude a supplied
ssl_context and TLS construction/verification customization that would be
ignored or would reconfigure the shared context. The installed urllib3
SSL_KEYWORDS and its context-construction path show which effective settings
need consideration. A short explicit check may be clearer than a helper if
the actual supported configuration set is small. It must consider proxy
manager settings as well as direct settings.

In cert_verify(), make CA omission depend on actual context ownership.
For ordinary urllib3 pools, the relevant sketch is:

```python
selected_context = conn.conn_kw.get("ssl_context")
uses_preloaded_context = (
    _preloaded_ssl_context is not None
    and selected_context is _preloaded_ssl_context
)
if verify is True and not uses_preloaded_context:
    cert_loc = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
    # Preserve previous existence checks and ca_certs / ca_cert_dir assignment.
```

Do not implement the above by assuming all third-party connection objects
expose conn_kw. The existing public subclass hook accepts custom connection
objects; either use a guarded lookup that defaults to the existing CA-loading
path or define ownership inside the adapter's own selection contract. A
missing cache must also not compare equal to a pool with no supplied context.

A lazy initializer can have this concrete shape:

```python
_preloaded_ssl_context = None
_preloaded_ssl_context_lock = threading.Lock()

def _get_preloaded_ssl_context():
    global _preloaded_ssl_context
    with _preloaded_ssl_context_lock:
        if _preloaded_ssl_context is None:
            path = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
            if not path or not os.path.exists(path):
                raise OSError(
                    "Could not find a suitable TLS CA certificate bundle, "
                    f"invalid path: {path}"
                )
            context = create_urllib3_context()
            context.load_verify_locations(path)
            _preloaded_ssl_context = context
        return _preloaded_ssl_context
```

This sketch preserves existing zipped-path resolution and publishes state
atomically after success. It retains the PR's assumption that certifi supplies
a bundle file. For compatibility with distributions providing a CA directory,
retain the former directory-aware loading behavior as appropriate. The lock
covers initialization, not network I/O. Backend creation can still fail for
the HTTPS request that requires it; it no longer fails package import.

All three sketches are proposals, not applied fixes. Eligibility must keep
request-specific client identities out of the cache and avoid mutable custom
verification settings. urllib3 also sets verification and ALPN fields during
connection setup, so merely labeling an SSLContext immutable would be false.
For a restricted plain configuration these writes use the same settings;
widening eligibility needs another audit. This is why indiscriminate sharing
or context-copying cannot be recommended as the structural simplification.

The larger optional cleanup is to compute CA/client-certificate selection and
validation once before choosing a pool, then have the compatibility hook
apply that resolved configuration. That could remove repeated path
classification between request kwargs and cert_verify(), but it must preserve
pool keys, error timing, and subclass semantics. It is not an additional
actionable finding about pre-existing duplication and should not delay the
minimal ownership fixes.

## Execution record

All tests were invoked from the unchanged clone root. Scratch files are in
`../review-tests/`; no bytecode, pytest cache, or fixture files were written
into the checkout. Common paths for the commands below are:

```sh
review_clone=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-001/clone
review_work=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-001/clone-work
review_python=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-001/clone-cache/venv/bin/python
```

The base source was exported using:

```sh
git show main:src/requests/adapters.py > "$review_work/review-tests/base_adapters.py"
```

The focused base/head reproduction command was:

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH="$review_clone/src" "$review_python" -m pytest \
  "$review_work/review-tests/test_context_ownership.py" \
  -q -o addopts= -p no:cacheprovider \
  --basetemp="$review_work/review-tests/tmp"
```

Result: 5 passed, 5 failed, in 1.33 seconds. Each base parameter passed and
each head parameter failed. The one client identity case used two successful
local verified HTTPS exchanges. Trustme generated a CA and fresh client/server
certificates; the test substituted the default CA path before loading each
adapter. Original Requests globals were restored by pytest monkeypatch.
Each module received its own fresh context, avoiding inter-test contamination.

The head's repository selection command was:

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH="$review_clone/src" "$review_python" -m pytest \
  tests/test_adapters.py tests/test_requests.py \
  -k 'ssl or verify or cert or tls' -q -o addopts= -p no:cacheprovider \
  --basetemp="$review_work/review-tests/repository-tmp"
```

Result: 15 passed, 3 failed, 312 deselected, in 2.60 seconds.

An additional baseline check used the exported base adapter through the
scratch `base_adapter_plugin.py`, replacing sys.modules["requests.adapters"],
requests.adapters, and requests.sessions.HTTPAdapter before collection:

```sh
PYTHONDONTWRITEBYTECODE=1 \
PYTHONPATH="$review_clone/src:$review_work/review-tests" "$review_python" -m pytest \
  tests/test_adapters.py tests/test_requests.py \
  -k 'ssl or verify or cert or tls' -q -o addopts= -p no:cacheprovider \
  -p base_adapter_plugin --tb=line --disable-warnings \
  --basetemp="$review_work/review-tests/base-repository-tmp" \
  > "$review_work/review-tests/base-repository-results.txt" 2>&1
```

Result: 15 passed, 3 failed, 312 deselected, in 2.22 seconds.
The identical failing cases are:

- test_pyopenssl_redirect: Missing Authority Key Identifier.
- test_different_connection_pool_for_tls_settings_verify_bundle_unexpired_cert:
  Missing Authority Key Identifier.
- test_different_connection_pool_for_mtls_settings: SSLV3_ALERT_CERTIFICATE_EXPIRED.

The first two match the packet's known environment limitations. The third
is an additional reproduced baseline fixture limitation at the current run
date. It is not evidence of a new PR failure. This baseline run isolates the
changed module while keeping the rest of the head package; it is not a full
base-tree test run. The only committed delta is this adapter file.

Installed dependency source was read directly from the pre-provisioned
virtualenv's `lib/python3.13/site-packages/urllib3/`. Its _version.py reports
2.8.0. The package's setup.py permits urllib3>=1.21.1,<3, but no other
dependency version was executed. No dependency downloads or internet
research were performed. Live-host tests and performance benchmarks were
outside the execution allowance.

The decisive findings are therefore verified in the supplied environment;
broader compatibility and concurrent scheduling behavior remain unexecuted.
The existing repository tests are not a clean pass, but their failures were
reproduced with the base adapter and excluded from actionable findings.

