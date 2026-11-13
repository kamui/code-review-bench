# Code review — psf/requests#6667 "Avoid reloading root certificates to improve concurrent performance"

Range reviewed: `8dd3b26bf59808de24fd654699f592abf6de581e..4089f3dc65f783beaa53cc032958ab625440d0ac` (`main...review-head`). Changed file: `src/requests/adapters.py`. No CLAUDE.md files apply to this path. Step 6 kept three issues, all validated in step 5.

## Issue 1: A client certificate from `cert=` is loaded into the shared module-level SSLContext and sent to unrelated hosts

- **Reason flagged:** bug (security)
- **File/lines:** `src/requests/adapters.py` lines 75-78 (`_preloaded_ssl_context` creation) and 94-95 (`pool_kwargs["ssl_context"] = _preloaded_ssl_context`), together with lines 102-109 (`cert_file`/`key_file` added to the same `pool_kwargs`).
- **Why it fails:** For `verify=True` (the default), every pool now gets the one process-wide `_preloaded_ssl_context`. urllib3 uses a supplied `ssl_context` as-is (`urllib3/connection.py` ~1031-1040). When the connection has a `cert_file`, `ssl_wrap_socket` calls `context.load_cert_chain(certfile, keyfile)` on that shared object (`urllib3/util/ssl_.py` ~429-433). So after one call such as `requests.get("https://a.example", cert=("client.pem", "client.key"))`, any later `verify=True` request in the process sends that client certificate to every server that requests one. This applies across Sessions, adapters and hosts, including requests made without `cert=`. Different `cert=` values also overwrite each other on the same context. Before the PR, `ssl_context` was `None`, so each connection got a fresh context. An offline repro with two local TLS servers confirmed it: a new Session with no `cert=` presented the earlier request's client certificate (CN `leaked-client-cert`) to the second server.

## Issue 2: A custom `ssl_context` set in an `HTTPAdapter.init_poolmanager` override is silently replaced when `verify=True`

- **Reason flagged:** bug (behavioural regression)
- **File/lines:** `src/requests/adapters.py` lines 94-95, reaching urllib3 through `connection_from_host(..., pool_kwargs=pool_kwargs)` at lines 395-401.
- **Why it fails:** A common pattern is to subclass `HTTPAdapter` and pass `ssl_context=my_ctx` to `PoolManager` in `init_poolmanager`, for a minimum TLS version, ciphers or a private CA. That puts the context in `PoolManager.connection_pool_kw`. `_urllib3_request_context()` now always adds `ssl_context=_preloaded_ssl_context` to the per-request `pool_kwargs` when `verify=True`. urllib3's `PoolManager._merge_pool_kwargs` lets per-request kwargs override `connection_pool_kw`, so the user's context is dropped. The result is a silent TLS-policy downgrade, and verification failures for hosts signed by a CA that only the user's context trusted. On `main`, `pool_kwargs` never contained `ssl_context`. An offline check showed the pool's context had `minimum_version` TLSv1_3 (the user's) on `main` but TLSv1_2 (the shared default) on head with `verify=True`. With `verify=False` or a CA-path string, the user's context is still used.

## Issue 3: `import requests` now fails on Python builds without the `ssl` module

- **Reason flagged:** bug (regression)
- **File/lines:** `src/requests/adapters.py` lines 75-78 (module-level `create_urllib3_context()` and `load_verify_locations(...)`).
- **Why it fails:** `create_urllib3_context()` now runs, unguarded, whenever `requests.adapters` is imported, and `requests/__init__.py` always imports that module. When `ssl` is unavailable, urllib3 sets `SSLContext = None`, and `create_urllib3_context()` raises `TypeError("Can't create an SSLContext object without an ssl module")` (`urllib3/util/ssl_.py` 223-224). Nothing in requests' import chain catches it, so `import requests` fails even for plain-HTTP use. Before the PR this worked: urllib3 guards its own ssl import, and `src/requests/__init__.py` on `main` explicitly tolerates a missing `ssl` (`try: import ssl / except ImportError: ssl = None`).
