# Second-pass ruling 1 supplement: is injecting pyOpenSSL after importing requests promised?

Added by the recording session on 2026-10-05 for the review of the nine. The dossier (`../../candidates/i-requests-6667/dossiers/N1.md`) established the behaviour at both commits. It did not establish whose interface the injection is or whether programs do it in this order.

## Whose interface

Read, `../../candidates/i-requests-6667/upstream/urllib3-pyopenssl-1.26.18.json` and `urllib3-pyopenssl-2.2.1.json`: `inject_into_urllib3()` is a public, documented urllib3 function at both versions. The module documentation says: "call `inject_into_urllib3` from your Python code before you begin making HTTP requests. This can be done in a `sitecustomize` module, or at any other time before your application begins using `urllib3`". The 2.2.1 text adds "**this module is no longer recommended**" and keeps the instruction. Neither project calls the function private or says not to call it after importing requests.

Read, the pinned diff: urllib3 is unchanged between the two commits. At the head requests builds a TLS context while it is imported, so the application's first use of urllib3 now happens inside `import requests`, where the application cannot see it.

## Whether programs do it in this order

Fetched 2026-10-05 (`search-inject-after-import.json`): a GitHub code search for Python files containing `inject_into_urllib3()` and `import requests` returns about 64,000 files; the first page is mostly vendored copies of requests' own `__init__.py`. Of the seven application files on that page, read one by one:

- four inject pyOpenSSL after `import requests`: `kappataumu/letsencrypt-cloudflare-hook` `hook.py`, `tohojo/bufferbloat-net` `scripts/invalidate-caches.py`, `anderlli0053/DEV-tools` `check_https.py`, `OmniLayer/omniEngine` `updateFees.py`;
- one injects before it (`ArcAster/eth_tracker`);
- one injects urllib3's SecureTransport backend after it (`explosion/wheelwright`), the same shape with another backend;
- one is a test tool that swaps the injection in and out (`gabrielfalcao/HTTPretty`).

Two of the four call it as `requests.packages.urllib3.contrib.pyopenssl.inject_into_urllib3()`, a spelling that can only run after requests is imported, and cite urllib3's security documentation. A search for that exact spelling returns 55 Python files.

Limits: two of the four guard the call with `if sys.version_info[0] == 2`, and requests at the head does not run on Python 2, so those two cannot be affected by this change. The files were not dated or run. The search shows the order is a real habit, mostly from the years when pyOpenSSL was the way to get SNI; it does not show how many programs still depend on it.
