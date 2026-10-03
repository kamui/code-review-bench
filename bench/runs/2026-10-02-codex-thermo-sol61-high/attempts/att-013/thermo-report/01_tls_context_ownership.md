# TLS context ownership and adapter policy

This subsystem carries the first two summary findings. Both concern the new assignment in `src/requests/adapters.py:94–95`: default verification now supplies a process-wide SSLContext through request-level pool kwargs. The object includes trust, protocol settings, hostname-verification settings, and client identity. Its reuse must therefore follow the effective TLS policy, rather than just the value of `verify`.

## Scope and measurements

The committed diff was inspected with `git diff main...review-head -- src/requests/adapters.py`, and the complete adapter was read with `nl -ba src/requests/adapters.py`. The manifest is one file, +28/−18. `git show main:src/requests/adapters.py | wc -l` reports 606 lines; `wc -l src/requests/adapters.py` reports 616. No file-size blocker applies.

The preloaded context is created at lines 75–78. Lines 94–95 put it into the request kwargs whenever verification is exactly True. This test is independent of `client_cert`, which is processed afterward at lines 102–109. Thus certificate-bearing pools and certificate-free pools receive the same object.

`HTTPAdapter._get_connection()`, lines 381–402, selects a proxy or direct manager and passes the TLS kwargs as overrides. `init_poolmanager()`, lines 221–245, is explicitly documented as an adapter-subclass extension point. `cert_verify()`, lines 300–305, now assumes a preloaded context exists whenever verification is boolean, although it receives neither the resolved manager policy nor a guarantee of context ownership. This makes reasoning about the trust store depend on a decision made in another function.

## Finding: shared context retains a client identity

The changed anchor is `src/requests/adapters.py:94–95`. urllib3’s installed `util/ssl_.py:429–433` calls `context.load_cert_chain(certfile, keyfile)` on the supplied context before wrapping the socket. Omitting `certfile` later does not clear that identity. `HTTPSConnectionPool._new_conn()` passes both client-certificate attributes and its `conn_kw` into each connection, so separate pools do not separate the underlying context.

The offline probe creates a fresh CA, server certificate, and client certificate with trustme. It installs that CA as the test default trust for each adapter version; on the head this substitutes a context preloaded with exactly that CA. This is fixture preparation to make default verification against a local server possible, not a mechanism that introduces the client-identity leak.

A local HTTPS server requests optional client authentication and records the peer certificate. The first Session sends `cert=(client.pem, client.key)` with default verification, then closes. The second Session has a fresh adapter and sends no `cert=`. Both requests establish new TLS connections. The base records client-certificate presence `[True, False]`; the head records `[True, True]`. The second head handshake carries the first Session’s identity after the first adapter has been closed.

The relevant retained probe is [test_tls_boundaries.py](../thermo-probes/test_tls_boundaries.py), `test_client_certificate_does_not_escape_session`. Its failure output is preserved in [boundary-output.txt](../thermo-probes/boundary-output.txt). This proves a sequential isolation failure. Concurrent certificate-profile interference is a credible consequence of the same shared mutable object, but a concurrent race was not measured and is not needed for the finding.

A new context per adapter alone is insufficient: two differently keyed certificate profiles within one adapter would still mutate the same identity. Clearing the certificate afterward is also the wrong ownership model, because other connections can be created between those mutations. The minimal remedy is to keep the reusable default context out of every request with a client certificate and retain the base’s per-connection context construction and default CA path for those requests. A future context cache for client identities would need ownership by complete TLS profile and fully configured identities before sharing; it is not required for this PR.

## Finding: request defaults defeat adapter TLS configuration

The changed anchor is again `src/requests/adapters.py:94–95`. The context is injected before the selected manager’s settings are considered. In the installed urllib3, `PoolManager._merge_pool_kwargs()`, lines 400–415, copies manager defaults and then replaces each key present in the request overrides. The new `ssl_context` therefore replaces an adapter-supplied context, even when that context was deliberately configured through `init_poolmanager()`.

A direct pool-resolution probe subclasses each adapter version, supplies a custom context in `init_poolmanager()`, and calls `_get_connection(..., verify=True)`. It checks object identity in `pool.conn_kw["ssl_context"]`. The base retains the supplied object; the head returns the global one. This check uses an `example.invalid` URL only to prepare pool metadata and performs no DNS lookup or network request.

The issue also affects protocol settings supplied without an explicit context. Installed urllib3’s `connection.py:1030–1040` calls `create_urllib3_context()` with `ssl_version`, `ssl_minimum_version`, and `ssl_maximum_version` only when `ssl_context is None`. The head’s pre-created context bypasses those inputs.

The handshake probe configures an adapter with `ssl_minimum_version=TLSv1_3` and a local server with `maximum_version=TLSv1_2`, using trusted, newly generated certificates. The base raises Requests SSLError and the server observes no HTTP request. The head successfully completes TLS and an HTTP request, causing the expected SSLError assertion to fail. This is an observed loss of an adapter security constraint, not just a concern inferred from overwritten metadata.

The probes are `test_custom_adapter_context_survives_default_verify` and `test_adapter_tls_minimum_is_enforced` in the retained boundary test. Repository documentation at `docs/user/advanced.rst:1017–1033` demonstrates passing TLS protocol configuration through a PoolManager in an adapter subclass. That example’s historical SSLv3 constant was not executed; the test uses current TLS version bounds.

Selection of a context must happen after selecting the manager and examining the policy it already owns. An optimization should preserve a supplied context, and it must not make configured protocol limits ineffective. The direct-manager failures are reproduced. The proxy-manager route receives the same override dictionary at adapter lines 394–396, so the equivalent precedence problem is supported by source inspection; no proxy TLS handshake was run.

## Worked code-judo proposal

The structural move is to model the cache as an optimization of one effective TLS policy, rather than a second owner of every request’s security settings. Keep `_urllib3_request_context()` concerned with URL parsing and explicit request settings. Let an adapter-owned resolver see the chosen manager’s defaults before selecting a reusable context. It can keep using the existing pool-kwargs dictionary; a general policy framework or new hierarchy is unnecessary.

The following is a decision sketch, not an applied patch. “Ordinary manager policy” must be defined against supported urllib3 options, including supplied contexts, protocol bounds, and hostname/fingerprint overrides that affect context use.

| Effective request policy | Context/trust source |
| --- | --- |
| Verification disabled | Existing unverified connection path |
| Explicit CA file or directory | Existing explicit-trust path |
| Default verification plus client certificate | Existing per-connection context and default CA bundle |
| Default verification plus custom manager TLS configuration | Preserve manager policy and the base trust-loading behavior |
| Default verification with ordinary manager policy and no client identity | Lazily obtained reusable default context |

The resolver belongs immediately before `manager.connection_from_host(..., pool_kwargs=...)`, where all inputs are available. Both proxy and direct manager branches can use the same resolved kwargs. It should validate selected file paths before creating a pool and avoid re-deciding the trust source after pool lookup. An eligibility check can be factored into this canonical boundary; scattering additional `verify is True` tests around `send()`, `cert_verify()`, and manager initialization would perpetuate the problem.

For the ordinary path, the resolved kwargs include the reusable context and omit `ca_certs` and `ca_cert_dir`. For a client-certificate or context-affecting manager path, default trust is represented by the extracted default CA path as at the base, while manager TLS configuration remains effective. Explicit trust selection retains the PR’s correct file-versus-directory distinction. This keeps the performance benefit where it is safe and gives the remaining policies a boring, established fallback.

Keep the documented `cert_verify()` hook callable. Consolidation must preserve subclass behavior rather than silently deleting an extension point. Its base implementation should use the effective connection configuration to avoid redundant loads, instead of assuming that the raw boolean `verify` proves a specific context has already been installed. Centralizing normalization and validation can then remove duplicated trust-policy branching. The exact compatibility refactor requires implementation and regression checks; no replacement code has been applied or verified here.

## Verification status

The five paired boundary probes run the head adapter and the exact base adapter source against identical Requests support modules and dependencies. Since adapters.py is the only changed file, this isolates the relevant change, while not pretending to reproduce an old dependency environment. All five base cases pass; five corresponding head cases fail. The two additional head fast-path/directory checks pass.

The default-fast-path test completes a verified local request, checks the actual pool’s context identity, and verifies that its CA-file and CA-directory attributes are None. Together with the inspected urllib3 conditional, this supports avoidance of the redundant explicit CA load. It does not quantify throughput or count every possible SSL call. The directory test verifies `ca_cert_dir` rather than `ca_certs` in the pool kwargs.

The environment is Python 3.13.15, urllib3 2.8.0, and OpenSSL 3.5.8. Results have not been verified across the entire supported urllib3/Python matrix. The context-identity override and client-chain mutation are directly observed in this environment, with comparative base controls.

## Commands and retained evidence

All pytest commands were run from the clone root, with bytecode and pytest cache writes disabled. Generated certificates, pytest scratch directories, and review sources reside in clone-work. Each selection was executed once with its recorded flags. No external network was used.

The base source preparation was:

```sh
git show main:src/requests/adapters.py > /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-013/clone-work/thermo-probes/base_adapters.py
```

The complete boundary selection was:

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-013/clone/src /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-013/clone-cache/venv/bin/python -m pytest /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-013/clone-work/thermo-probes/test_tls_boundaries.py -p no:cacheprovider --basetemp=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-013/clone-work/thermo-probes/pytest-boundaries -q -s
```

It completed in 2.88 seconds with five failed, seven passed. The failed head cases cover the two ownership/configuration findings above and the initialization finding in [02_tls_initialization.md](02_tls_initialization.md).

