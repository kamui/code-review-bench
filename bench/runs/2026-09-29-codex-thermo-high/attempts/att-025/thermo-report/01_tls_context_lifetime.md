# TLS context lifetime and credential isolation

## Scope

The reviewed change is limited to `src/requests/adapters.py` in `8dd3b26bf59808de24fd654699f592abf6de581e..4089f3dc65f783beaa53cc032958ab625440d0ac`. The diff adds a module-level SSL context preloaded with the default CA bundle, passes it into every pool built with `verify=True`, and stops setting `ca_certs`/`ca_cert_dir` for boolean verification.

## Finding: the shared default context can leak client certificates across requests

The module creates `_preloaded_ssl_context` once at lines 75–78. `_urllib3_request_context()` places that same object into `pool_kwargs["ssl_context"]` whenever `verify is True` at lines 94–95, independently of whether `client_cert` is present. The same function separately puts each request's client certificate paths into the pool kwargs at lines 102–108. The send path then selects that pool and calls `cert_verify()` before urllib3 opens a TLS connection.

In the installed urllib3 2.8.0 implementation, `urllib3.util.ssl_.ssl_wrap_socket()` assigns `context = ssl_context`; when `certfile` is set, it calls `context.load_cert_chain(certfile, keyfile)`. Thus a verified request with a client certificate changes the module-global context. That context is then used by unrelated `verify=True` requests, so credentials can persist across requests and be presented to a server for which the caller did not select that client identity. Different client-certificate requests can also race while changing the same SSL context. Pool separation does not isolate the context because pool kwargs point to the same object.

The issue is introduced by the new sharing boundary, not by the existence of client certificates: before this change, urllib3 created a context for a connection and loaded that connection's client certificate into its own context. The new default-CA optimization makes the same mutable context cross request and pool boundaries.

## Worked code-judo proposal

Make context ownership follow TLS identity. Keep the current preloaded singleton only for the common `verify=True, cert=None` case. For mTLS, create a separate context configured for that certificate identity before it is handed to urllib3: load the default CA roots, load the selected client chain once, then pass that prepared context without per-connection `cert_file`/`key_file` mutation. Ensure pool configuration still distinguishes client-certificate identities. This keeps the fast shared default path, removes cross-identity mutation, and makes the context's lifetime and trust/credential contents agree.

The implementation should make the branch explicit at the context-construction boundary rather than sharing the root-only singleton and hoping urllib3's later connection setup leaves it unchanged. If caching certificate-specific contexts is needed, cache only fully configured contexts keyed by the complete client-certificate identity; do not load or replace credentials on a context after it has been shared with connections.

## Actionable remediation

Restrict `_preloaded_ssl_context` to requests without client certificates. Give requests with `cert=` an isolated, fully initialized certificate-specific context, preserving the normal CA bundle for server verification and ensuring different identities cannot reuse or mutate one another's context. Add regression coverage for a client-certificate request followed by a verified request without `cert=`, plus concurrent requests using different client certificates.

## Verification status and evidence

The following inspection commands were used:

```text
git diff --find-renames --unified=80 main...review-head -- src/requests/adapters.py
nl -ba src/requests/adapters.py | sed -n '68,108p;282,330p;505,540p'
PYTHONPATH=<clone>/src <cache>/venv/bin/python -c 'import inspect; from urllib3.util.ssl_ import ssl_wrap_socket; print(inspect.getsource(ssl_wrap_socket))'
```

The installed urllib3 reported version `2.8.0`; its inspected source loads a supplied `certfile` into the supplied context via `context.load_cert_chain(certfile, keyfile)`. No tests were run, and no files in the clone were modified. The behavior follows from the current source-level object sharing and mutation path; the proposed regression tests have not been executed.
