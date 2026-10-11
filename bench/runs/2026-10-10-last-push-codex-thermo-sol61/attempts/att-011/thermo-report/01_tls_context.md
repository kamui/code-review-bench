# TLS context ownership and lifecycle

## Scope and measurements

This is a single primary review. The frozen thermo-nuclear-code-quality-review skill was read from the supplied work-directory path. It contains no required child-review call, so no agents were started. Repository guidance and ambient configuration were not loaded as review instructions. The packet’s historic review comments were not used as finding evidence.

The committed change was inspected with `git diff main...review-head`. The working checkout was initially clean, HEAD was `4089f3dc65f783beaa53cc032958ab625440d0ac`, and main was `8dd3b26bf59808de24fd654699f592abf6de581e`. Commands `git show main:src/requests/adapters.py | wc -l` and `wc -l src/requests/adapters.py` measured 606 and 616 lines. The change is one file, +28/−18. A module split would add indirection without solving the ownership problem.

Source inspected: the complete adapters module; Requests import setup in `src/requests/__init__.py`; `extract_zipped_paths` in utils; session dispatch and environment merging; the relevant adapter, TLS, and local-server tests; and installed urllib3 connection, pool-manager, and SSL utility code. Local installed metadata identifies urllib3 2.8.0, and the virtualenv library directory is Python 3.13. The supplied environment was used without installing anything.

## Client identity ownership

Finding anchor: `src/requests/adapters.py:94–95`. Related code: lines 102–109 place client-certificate paths in separate pool kwargs, and lines 323–338 validate and install them on the pool. Those paths affect the pool key, but all `verify=True` keys still contain the same context object.

The installed urllib3 `util/ssl_.py:407–433` chooses the supplied context and calls `context.load_cert_chain(certfile, keyfile)` whenever a certfile is present. It never removes the old identity when certfile is absent. The SSL context stores both trust settings and client credentials. Sharing roots by sharing that whole mutable object therefore shares more authority than the optimization intends.

The scratch test `test_client_identity_does_not_leak_to_unrelated_session` constructs a new CA and client certificate with trustme and runs a loopback HTTPS server requesting optional client authentication. The base adapter’s default CA path points to the generated CA. The head’s cached context is replaced with a normally constructed context trusting that same CA. This substitution makes the local server a trusted default-root server; it does not change routing, client-certificate selection, pool construction, or urllib3’s certificate mutation behavior.

Two separate Requests sessions make successful requests with `verify=True`. The first supplies a client PEM; the second supplies no client certificate. Closing each session ensures that this observation is about fresh TLS connections, not reuse of the first socket. Both use `trust_env=False` and bounded request timeouts. The server records whether a peer certificate is present. Base records `[True, False]`; head records `[True, True]`. The second session unintentionally authenticates with the first session’s credentials.

Verification status: demonstrated sequentially on base and head in the installed runtime. No concurrency race is needed for this finding. Concurrent requests choosing different client identities can also mutate the same object, but which identity wins was not tested and is not claimed as a measured result. Cross-host disclosure follows from the global ownership model, while the reproduction deliberately uses one local server to avoid live-host dependencies.

The direct remedy is to use the cached context only when no client certificate is supplied. Requests with a client certificate should keep urllib3’s independently constructed context and the original default CA loading. Caching per client identity is not required to achieve the stated common-case optimization and introduces invalidation and mutation questions. Do not attempt to “clear” or swap certificates around a handshake on the singleton.

## Adapter policy precedence

Finding anchor: `src/requests/adapters.py:94–95`. The documented subclass boundary is `init_poolmanager` at lines 221–245. The direct and proxy selection paths at lines 394–402 pass the request kwargs to their manager.

urllib3 `PoolManager._merge_pool_kwargs` copies manager defaults and applies request overrides. A supplied `ssl_context` therefore wins over one configured by the adapter. Previously the request helper supplied no context for `verify=True`, so the configured manager context remained selected.

The scratch test `test_custom_adapter_context_remains_selected` defines a minimal adapter subclass whose `init_poolmanager` supplies a custom context. It prepares an HTTPS request and examines the pool returned by `_get_connection(request, verify=True)`. The base pool retains the exact custom context object; the head pool contains the module singleton. This is an ordinary documented customization, not mutation of a private global.

Verification status: the object-selection regression is demonstrated against base and head. No remote handshake or particular cipher suite was tested. Consequences for ciphers, protocol policy, and preloaded credentials follow directly from discarding the configured object. Proxy precedence is established by inspecting the same override path; a proxy server was not executed.

The remedy must preserve manager authority. Choose the default cache only if the selected direct/proxy manager has no context or TLS overrides that require independently configured state. For example, a manager configured with an explicit protocol version must not silently receive a previously built default context that ignores that policy. Merely changing `cert_verify` to skip CA loading for all booleans will still be wrong when custom contexts are honored. Inspect the actual selected context when deciding whether the default roots are already loaded.

## Import and resource lifecycle

Finding anchor: `src/requests/adapters.py:75–78`. The module eagerly constructs a context and loads the bundle before any adapter or request exists. `src/requests/__init__.py` imports sessions/API, which imports this transport module. Its existing SSL fallback handling explicitly tolerates import failures before adapter import.

At the base, `cert_verify` resolved the default bundle only for truthy verification on an HTTPS URL. HTTP transport and explicit alternate CA paths did not require successful loading of that default resource. The new module initializer removes that separation.

The import test loads base or head adapter source under a temporary module name in the Requests package after patching the imported utility’s default bundle path to a nonexistent file. Base imports and constructs an adapter; head raises `FileNotFoundError` at line 76. The second parameter sets urllib3’s `SSLContext` implementation to None. Base imports and constructs an adapter; head raises `TypeError: Can't create an SSLContext object without an ssl module` at line 75.

Verification status: both dependency-failure states are demonstrated as isolated adapter-import probes. The no-SSL probe simulates the unavailable implementation; it is not a complete run on a Python distribution lacking SSL. It independently shows that the new initializer, rather than request dispatch, demands that implementation. The missing-CA probe does not alter any certifi installation. Module loading occurs in the existing process to avoid destructive package edits.

The remedy is lazy, synchronized construction of the default context, invoked only for default verified HTTPS. Build and load a candidate before publishing it. Failed initialization must not leave a half-configured cache. Keep the canonical zip extraction utility and the existing invalid-bundle error contract. Do not swallow certificate-loading errors and silently use unrelated trust roots.

## Worked code-judo proposal

The useful reframing is “reuse the anonymous default TLS policy.” It is not “reuse a context whenever a boolean is true.” This puts ownership and precedence at the selection boundary and makes request dispatch consume a resolved decision.

A minimal restructuring can retain `_urllib3_request_context` for host and request arguments, introduce one adapter method that resolves effective manager TLS policy, and leave the public `cert_verify` hook called exactly once. Select the direct/proxy manager before choosing a context so its configured defaults are available. Derive the pool key and connection attributes from the same resolved CA source; do not duplicate file/directory decisions in two unrelated flows.

This sketch illustrates the lazy cache; it is a proposal, not an applied or tested patch:

```python
_default_context = None
_default_context_lock = Lock()

def _default_ca_path():
    path = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
    if not path or not os.path.exists(path):
        raise OSError(
            "Could not find a suitable TLS CA certificate bundle, "
            f"invalid path: {path}"
        )
    return path

def _get_default_context():
    global _default_context
    with _default_context_lock:
        if _default_context is None:
            candidate = create_urllib3_context()
            candidate.load_verify_locations(_default_ca_path())
            _default_context = candidate
        return _default_context
```

A selection sketch expresses the important ownership decisions:

```python
# manager is the already selected direct or proxy manager.
# tls_overrides includes context, protocol/cipher, and hostname/fingerprint
# policy that would make use of the default context inappropriate.
if scheme == "https" and verify is True:
    if client_cert is None and not tls_overrides(manager.connection_pool_kw):
        pool_kwargs["ssl_context"] = _get_default_context()
    # Otherwise inherit the configured manager context, or let urllib3
    # create an independent context. cert_verify loads default roots.
```

The `tls_overrides` check needs an explicit set of the supported manager TLS options rather than arbitrary generic introspection. Existing HTTP pool-size or blocking options must not disable the optimization. A new wrapper that merely forwards kwargs would not earn its keep; this method earns its place only by resolving the previously scattered ownership and precedence rules.

For the request-time hook, retain the original certificate/file validation flow but skip default-root installation only if the connection is actually using the initialized default cache and the request has no client identity. Checking the pool’s `conn_kw["ssl_context"]` against the cache is a small, explicit bridge to urllib3’s representation. Keep that bridge local to adapter policy. Do not instantiate the lazy cache merely to compare identities.

For custom CA sources, a single resolver should validate and return either `{"ca_certs": path}` or `{"ca_cert_dir": path}`. Use it for pool identity before pool construction and for connection configuration without reclassifying the path later. Preserve timing and messages of path-validation errors and public subclass hooks. This can delete the duplicate directory branch and the multi-line “boolean implies preloaded context” assumption in `cert_verify`, without a large new policy framework or a module split.

The sketches intentionally do not specify a complete compatibility patch: subclass overrides, supported urllib3 versions, and custom TLS option combinations require tests before implementation is accepted. No remedy was applied to the checkout.

## Test commands and results

All pytest commands ran from the clone root with these environment settings and executable:

```sh
PYTHONDONTWRITEBYTECODE=1
PYTHONPATH=<clone>/src
<clone-cache>/venv/bin/python -m pytest <selection> ...
```

The paths abbreviated above are the supplied attempt-local clone, clone-cache, and clone-work roots. Commands disabled pytest cache writes with `-p no:cacheprovider` and used `--basetemp=<clone-work>/...`. Each selection/flag set ran once, and every command completed well within five minutes. Only local fixture servers communicated over sockets.

Existing selection:

```text
tests/test_requests.py
-k 'ssl or verify or cert_bundle or mtls or invalid_ca_certificate'
-p no:cacheprovider --basetemp=<clone-work>/pytest-existing -q
```

Result: 13 passed, 3 failed, 313 deselected in 2.87 seconds. The two packet-known failures were `test_pyopenssl_redirect` and `test_different_connection_pool_for_tls_settings_verify_bundle_unexpired_cert`, with Missing Authority Key Identifier. The additional failure was `test_different_connection_pool_for_mtls_settings`, with SSLV3_ALERT_CERTIFICATE_EXPIRED on its first, unverified request.

Scratch regression selection:

```text
<clone-work>/test_review_tls.py
-p no:cacheprovider --basetemp=<clone-work>/pytest-probes -q
```

Result: 4 passed, 4 failed in 1.21 seconds. All four base cases passed. The head failed the custom-context assertion, the identity-isolation assertion, and the two import-resource cases. The scratch source is preserved at [test_review_tls.py](../test_review_tls.py), and its unchanged base source snapshot is [base_adapters.py](../base_adapters.py), extracted with `git show main:src/requests/adapters.py`.

Base-fixture check:

```text
<clone-work>/test_review_base_fixture.py::test_existing_mtls_fixture_on_base
-p no:cacheprovider --basetemp=<clone-work>/pytest-base-fixture
-q --tb=short --disable-warnings
```

Result: the same certificate-expired failure in 0.28 seconds. This scratch test replaces the session constructor’s adapter class with the extracted base adapter and calls the existing mTLS test unchanged. The changed file is the only source difference between base and head, so this comparison isolates its effect while leaving repository files untouched. Source is preserved at [test_review_base_fixture.py](../test_review_base_fixture.py).

## Limits and review judgment

No live-host tests, performance benchmark, older urllib3 environments, packaged zip deployment, or actual SSL-less Python environment were executed. The reported behavioral differences are backed by base/head probes in the provisioned runtime; additional platform conclusions are marked as source inference. There is no independent or alternate-model review.

The directory support and use of existing SSL/zip helpers are sound. Naming the cache private is useful API hygiene but does not prevent urllib3 from mutating it. The requested optimization can stay small: reuse one lazily constructed context only for anonymous default TLS, respect adapter policy, and retain an independent context for client identity. Those ownership constraints should be made concrete before approval.

