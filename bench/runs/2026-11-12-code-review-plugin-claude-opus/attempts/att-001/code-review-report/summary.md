# Code review — psf/requests#6667 "Avoid reloading root certificates to improve concurrent performance"

Range: `8dd3b26bf59808de24fd654699f592abf6de581e..4089f3dc65f783beaa53cc032958ab625440d0ac` (`main...review-head`)

Step 1: PR is not a draft and needs review. Step 2: no CLAUDE.md files apply. Step 4: CLAUDE.md agents found no issues; bug agents raised 2 issues; both were confirmed in step 5 and kept in step 6.

## Issues found: 2

### 1. A client certificate is loaded into the shared module-level SSLContext and leaks to unrelated requests

- **Reason flagged:** security / bug
- **File and lines:** `src/requests/adapters.py` lines 75-78 (`_preloaded_ssl_context`), 94-95 (`pool_kwargs["ssl_context"] = _preloaded_ssl_context`), and 102-109 (`cert_file`/`key_file` pool kwargs)
- **Why it fails:** When `verify=True`, every connection pool in the process gets the same `_preloaded_ssl_context` object. If a request also passes `cert=`, urllib3 uses that object as-is: `ssl_wrap_socket` does `context = ssl_context` and then `context.load_cert_chain(certfile, keyfile)`, with no copy. The client certificate and private key are therefore installed permanently on the process-wide context. Later `verify=True` requests from any session that pass no `cert` will present that identity when a server asks for a client certificate. Concurrent requests with different client certs also race, and the last `load_cert_chain` call wins. Before this PR, `ssl_context` was None, so urllib3 built a fresh context per connection and the client cert stayed with that connection.

### 2. A custom `ssl_context` set by an `HTTPAdapter` subclass is silently overridden when `verify=True`

- **Reason flagged:** bug
- **File and lines:** `src/requests/adapters.py` lines 94-95
- **Why it fails:** A common pattern is an adapter subclass that calls `init_poolmanager(..., ssl_context=my_ctx)` (or does the same in `proxy_manager_for`). That context is stored in `PoolManager.connection_pool_kw`. `_urllib3_request_context` now always puts `ssl_context=_preloaded_ssl_context` into the per-request `pool_kwargs` when `verify=True`, which is the default. urllib3's `PoolManager._merge_pool_kwargs` lets per-request kwargs overwrite `connection_pool_kw`, so the user's context is replaced on every request, including through proxies. Custom ciphers, minimum TLS versions, trust stores, and client certs loaded on the context are all dropped. An offline check confirmed it: the user's context survives at base (`main`) but is replaced at head for `verify=True`, with or without a proxy.
