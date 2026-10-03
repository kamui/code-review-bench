# TLS context ownership and adapter policy

## Scope and judgment

This subsystem contains F1 and F2 from [summary.md](summary.md).
Both are ownership defects caused by treating a mutable context as an
unconditional process-wide default. The common cache can retain its performance
benefit once its eligibility boundary is explicit.

## Source evidence

At head `src/requests/adapters.py:75–78` constructs one SSL context.
At lines 94–95, all `verify=True` requests receive that object.
Lines 102–109 independently attach `cert_file` and `key_file` to pool kwargs,
so supplying a client certificate does not exclude use of the shared context.

`HTTPAdapter._get_connection()`, lines 377–404, passes those kwargs to either the
direct pool manager or the selected proxy manager. Its `send()` calls
`cert_verify()` after obtaining the pool. Lines 323–338 retain client
certificate assignment and path validation.

The installed urllib3 source is under
`../clone-cache/venv/lib/python3.13/site-packages/urllib3/`.
`util/ssl_.py:407–433` uses the supplied context and calls
`context.load_cert_chain(certfile, keyfile)` whenever a cert file is supplied.
It does not remove a previously loaded certificate when no cert file is supplied.
`connection.py:1030–1056` also mutates verification and hostname settings on
a supplied context. Renaming a shared object with a leading underscore does not
prevent these internal mutations.

The pool manager keys include SSL context and certificate file inputs, but
distinct keys do not clone the SSL context. Pool partitioning therefore cannot
restore authentication-state isolation.

For F2, `HTTPAdapter.init_poolmanager()`, lines 221–245, explicitly exposes
`**pool_kwargs` for adapter subclasses. In urllib3,
`PoolManager.connection_from_host()` merges per-request kwargs with manager
defaults. `_merge_pool_kwargs()`, lines 390–413, copies the defaults and then
overwrites keys supplied by the request. The new `ssl_context` key therefore
wins over an adapter-supplied context.

The base request helper supplied `cert_reqs` and certificate paths but did not
supply a default SSL context. The custom context was consequently retained.
The repository's `docs/user/advanced.rst:1010–1033` also demonstrates the
adapter extension point for TLS protocol configuration.

A supplied context bypasses urllib3's normal context-construction branch.
Thus merely respecting an explicit `ssl_context` is insufficient as a complete
remedy: adapter-defined `ssl_version`, minimum/maximum TLS versions, and other
context-construction inputs need their normal opportunity to take effect too.
The executed regression directly verifies explicit context precedence; the
broader construction-settings consequence follows from the inspected branch.

## F1: verified client identity crosses Session boundaries

The scratch reproduction is
[../test_review_tls.py](../test_review_tls.py).
It loads the base adapter from
[../base_adapters.py](../base_adapters.py), copied directly by
`git show main:src/requests/adapters.py`, and independently loads the unchanged
head file under a test-only package name. Relative imports use the existing
Requests package; the only changed repository module is the adapter.

The test generates separate trustme server and client CAs and a client identity.
The server presents a valid localhost certificate and optionally requests a
client certificate. Both requests verify the server.
The head's test-private preloaded context contains the server CA; the base's
default bundle path points at the same CA file. This adjustment models a
default trusted server without changing public trust files or bypassing
verification.

The first Session sends a request with a client certificate.
That Session is closed. A new Session and adapter then send a request to a new
connection with `cert=None`. The server records the peer certificate during
both handshakes.

The base returns a client identity on the first connection and no identity on
the second. The head returns the client identity on both.
Both HTTP responses are 200 and the server reports no handshake errors.
The failed assertion is:
`Second session unexpectedly sent the first client's certificate`.

This establishes sequential contamination. It does not require a race,
connection reuse, a shared Session, or external access to the private context.
Concurrent requests carrying different identities can additionally compete
over `load_cert_chain()`; that is a source-derived consequence, not a separate
executed concurrency claim.

This is an authentication-boundary failure, rather than exposure of private-key
bytes. A server that requests and accepts the previously loaded identity can
observe or authenticate it when the caller selected no client certificate.

## F2: adapter-supplied SSL context is discarded

The scratch `CustomAdapter` calls its parent `init_poolmanager()` with a private
`ssl_context`. It then obtains an HTTPS pool through `_get_connection()`
with `verify=True`.

The base pool's `conn_kw["ssl_context"]` is the exact custom object.
The head pool's context is the newly imposed preloaded object.
No network connection is attempted; `example.invalid` is only parsed as the
pool's hostname.

The test proves object replacement. The effect on trust and cipher choices is
a direct consequence because the configured object no longer reaches urllib3's
TLS wrapping path. The reproduction does not attempt a matrix of custom cipher
or protocol handshakes.

## Verification commands and results

The commands were run from the clone root. Bytecode and pytest cache writes were
disabled so the checkout remained untouched. All pytest temporary paths and logs
were in the work directory.

```sh
git diff main...review-head
git show main:src/requests/adapters.py > ../clone-work/base_adapters.py
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH="$PWD/src" \
  ../clone-cache/venv/bin/python -m pytest \
  ../clone-work/test_review_tls.py -q -s -p no:cacheprovider \
  --basetemp=../clone-work/pytest-review-tmp
```

The actual invocation used absolute paths equivalent to those above.
Output was saved as [../review-tests.log](../review-tests.log).

Result: 3 failed, 4 passed in 0.29 seconds.
The three base behavioral cases pass; the three head cases fail.
The dependency probe passes and reports urllib3 2.8.0 and OpenSSL 3.5.8.
These installed sources are supporting execution evidence, not upstream
discussions or later review findings.

## Code-judo proposal: defaults belong to effective TLS policy

The existing request-context helper should build one effective policy that
describes the pool's actual TLS behavior. Its inputs must include the selected
manager's configured policy, because a request helper that sees only `verify`
cannot distinguish an ordinary default from adapter customization.

A behavior-preserving outline is:

```python
# Outline: preserve existing subclass hooks and Requests error semantics.
manager = select_existing_direct_or_proxy_manager(request, proxies)
host_params, tls_kwargs = normalize_request_tls(request, verify, cert)

if is_default_verified_https(host_params, verify):
    if cert or manager_has_custom_tls_policy(manager):
        # Retain urllib3's per-connection context, or the manager's context.
        # Preserve Requests' normal default CA source on this path.
        tls_kwargs["ca_certs"] = validated_default_ca_path()
    else:
        tls_kwargs["ssl_context"] = get_default_ssl_context()

pool = manager.connection_from_host(**host_params, pool_kwargs=tls_kwargs)
```

This is a design outline, not a patch validated against every subclass.
The helpers represent policy responsibilities; they need not all become public
functions or separate wrapper layers. Extend the existing
`_urllib3_request_context()` and keep the flow direct.

For a user-provided bundle or directory, preserve its request-specific path in
the normalized kwargs and keep it distinct in the pool key.
For disabled verification, retain the current separate policy.
For HTTP, no default TLS context is necessary.
For a supplied adapter context, do not replace it. Preserve existing CA-loading
behavior rather than silently discarding or augmenting trust differently.

The key simplification is that the later certificate hook no longer has to
guess which CA state exists from `verify is True`.
Where compatibility permits, validate normalized paths before pool lookup and
let both the pool key and connection consume the same values.
Keep `cert_verify()` callable for subclasses; if a legacy path supplies a pool
without the known preloaded context, it still needs the default CA path.

The smallest safe F1 fix is to retain fresh contexts for all client-certificate
requests. Cache contexts per client identity only if a separately justified
need arises, and build each fully before sharing it. A lock around loading the
same global identity would serialize mutation but leave the wrong identity
installed for later requests.

Moving the common object to each adapter alone is insufficient: one adapter
can still service requests with different client identities.
The correct partition is TLS policy and identity, not just adapter lifetime.

For F2, inspect the already selected direct or proxy manager before injecting
a default. Respect both explicit contexts and construction settings.
Do not force callers to override a private request helper merely to preserve a
documented adapter extension point.

## Required validation of a remedy

Retain the valid-certificate two-Session test and add independent Sessions using
two different client identities concurrently. Include an intervening request
with no client certificate to detect state retention.

Exercise a custom adapter SSL context and a proxy-manager context.
Exercise TLS construction settings without an explicit context.
Check ordinary default requests still reuse a preloaded context and that
different verify and certificate settings continue to select appropriate pools.

No proposed restructuring or remedy was applied to the checkout.

