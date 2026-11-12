# Sources

All fetched on 2026-11-08 (UTC) with `gh` 2.x, authenticated, unless the line says otherwise.

## Project documentation and source at the head

- `project-docs-search.txt`: `rg` over a `git archive` of psf/requests at `4089f3dc65f783beaa53cc032958ab625440d0ac`; the two queries are printed in the file.

## N1: a client certificate whose load fails, on the shared context

- `N1-owner-docs-cpython-v3.12.3-ssl.rst`: `gh api 'repos/python/cpython/contents/Doc/library/ssl.rst?ref=v3.12.3' -H 'Accept: application/vnd.github.raw'`
- `N1-owner-docs-openssl-3.3.0-SSL_CTX_use_certificate.pod`: `gh api 'repos/openssl/openssl/contents/doc/man3/SSL_CTX_use_certificate.pod?ref=openssl-3.3.0' -H 'Accept: application/vnd.github.raw'`
- `N1-owner-docs-openssl-3.3.0-openssl-threads.pod`: `gh api 'repos/openssl/openssl/contents/doc/man7/openssl-threads.pod?ref=openssl-3.3.0' -H 'Accept: application/vnd.github.raw'`
- `N1-maintainers-q1.json`: `gh api --method GET search/issues -f q='repo:psf/requests load_cert_chain created:<=2024-05-15' -f per_page=50`
- `N1-maintainers-q2.json`: `... -f q='repo:psf/requests "client certificate" thread created:<=2024-05-15'`
- `N1-maintainers-q3.json`: `... -f q='repo:psf/requests KEY_VALUES_MISMATCH created:<=2024-05-15'`
- `N1-maintainers-q4.json`: `... -f q='repo:psf/requests SSLContext "thread safe" created:<=2024-05-15'`
- `N1-maintainers-issue-3109.json`: `gh api repos/psf/requests/issues/3109`
- `N1-public-code-q1.json`: `gh api --method GET search/code -f q='"cert=(" "ThreadPoolExecutor" "requests.get" language:python' -f per_page=30`; GitHub answered HTTP 503, "too many shards failed".
- `N1-public-code-q2.json`: `gh api --method GET search/code -f q='ThreadPoolExecutor "cert=(" requests language:python' -f per_page=30`
- `N1-public-code-q2-read.json` and `N1-public-code/`: for ten hits, the last commit touching the file at or before 2024-05-15T09:56:42Z and the file at that commit, fetched as for N2 below
- `N1-later-q1.json`: `... -f q='repo:psf/requests load_cert_chain created:>2024-05-15'`
- `N1-later-q2.json`: `... -f q='repo:psf/requests segfault created:>2024-05-15'`

## N2: an ssl_context given through proxy_manager_for

- `N2-owner-code-urllib3-2.2.1-poolmanager.py`: `gh api 'repos/urllib3/urllib3/contents/src/urllib3/poolmanager.py?ref=2.2.1' -H 'Accept: application/vnd.github.raw'`
- `N2-owner-docs-urllib3-2.2.1-advanced-usage.rst`: `gh api 'repos/urllib3/urllib3/contents/docs/advanced-usage.rst?ref=2.2.1' -H 'Accept: application/vnd.github.raw'`
- `N2-maintainers-q1.json`: `gh api --method GET search/issues -f q='repo:psf/requests proxy_manager_for created:<=2024-05-15' -f per_page=50`
- `N2-maintainers-q2.json`: `... -f q='repo:psf/requests proxy ssl_context created:<=2024-05-15'`
- `N2-maintainers-issue-<n>.json` and `N2-maintainers-issue-<n>-comments.json` for 2048, 2049, 6109, 4046, 5006 and 2118: `gh api repos/psf/requests/issues/<n>` and `gh api 'repos/psf/requests/issues/<n>/comments?per_page=100'`
- `N2-maintainers-pull-2049.json`: `gh api repos/psf/requests/pulls/2049` (merged 2014-06-23T19:11:13Z)
- `N2-maintainers-6109-linked-adapters.py`: the file the maintainer linked in #6109, `gh api 'repos/psf/requests/contents/requests/adapters.py?ref=2d5517682b3b38547634d153cea43d48fbc8cdb5' -H 'Accept: application/vnd.github.raw'` (line 167 is `def proxy_manager_for`)
- `N2-public-code-q1.json`: `gh api --method GET search/code -f q='"def proxy_manager_for" "ssl_context" language:python' -f per_page=30`
- `N2-public-code-q1-read.json` and `N2-public-code/`: for ten hits, the last commit touching the file at or before 2024-05-15T20:07:26Z (`gh api 'repos/<repo>/commits?path=<path>&until=2024-05-15T20:07:26Z&per_page=1'`) and the file at that commit (`gh api 'repos/<repo>/contents/<path>?ref=<sha>' -H 'Accept: application/vnd.github.raw'`)
- `N2-later-q1.json`: `... -f q='repo:psf/requests proxy_manager_for created:>2024-05-15'`
- `N2-later-q2.json`: `... -f q='repo:psf/requests proxy ssl_context created:>2024-05-15'`

## Later records for both

- `later-issue-6726.json`, `later-issue-6726-comments.json`, `later-issue-6736.json`, `later-issue-6736-comments.json`: `gh api repos/psf/requests/issues/<n>` and its comments, as above.
- The release notes of 2.32.0, 2.32.3 and 2.32.5 are read from `HISTORY.md` in the `v2.32.4` and `v2.32.5` tags (`git show <tag>:HISTORY.md`).
