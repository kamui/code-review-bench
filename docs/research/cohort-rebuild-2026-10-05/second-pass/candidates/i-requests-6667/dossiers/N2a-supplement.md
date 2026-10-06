# N2a supplement: whose contract is `DEFAULT_CIPHERS`?

Added by the recording session on 2026-10-05, after the user asked during ruling 2 whether this is a urllib3 bug, whether the affected users follow the documentation, and whether the change breaks a contract. Every fact here was fetched in that session and was available before the merge of 2024-05-15 unless marked later.

## Whose behaviour changed

Read: urllib3 1.26.18's `create_urllib3_context()` reads `DEFAULT_CIPHERS` each time it is called (`../upstream/urllib3-ssl-1.26.18.json`). urllib3 is the same at both commits. At the base requests left urllib3 to call the factory per connection; at the head requests calls it once at import. The change in behaviour comes from requests.

## What is documented

- Read: requests' documentation at the head (`../upstream/requests-docs-advanced-head.rst`) does not mention `DEFAULT_CIPHERS`, ciphers, or `requests.packages`. Its only documented way to change TLS settings is a custom `HTTPAdapter` ("Example: Specific SSL Version", line 1017).
- Read: urllib3 1.26.18's advanced-usage page does not mention `DEFAULT_CIPHERS` (`../upstream/urllib3-advanced-1.26.18.json`).
- Read: urllib3's 2.0 changelog, line 326 of `../upstream/urllib3-CHANGES-10ae8a9c.rst`: "Removed ``DEFAULT_CIPHERS``, ``HAS_SNI``, ``USE_DEFAULT_SSLCONTEXT_CIPHERS``, from the private module ``urllib3.util.ssl_``". urllib3 2.0 was released in May 2023. On urllib3 2.x the assignment creates an unused attribute at both commits.
- Read: requests' `setup.py` at the head requires `urllib3>=1.21.1,<3`, so 1.26.x is supported and a fresh install gets 2.x.

## What the requests maintainers said about it before the merge

Read, from `../upstream/issue-2573-comments.json`, `issue-3608-comments.json`, `issue-3833-comments.json`, `issue-1308-comments.json`, found with `search-default-ciphers-before-merge.json`:

- 2015-05-13, Cory Benfield (Lukasa, member), issue 2573: "For the moment, setting a custom cipher suite is done by changing `requests.packages.urllib3.util.ssl_.DEFAULT_CIPHERS` (or `requests.packages.urllib3.contrib.pyopenssl.DEFAULT_CIPHER_LIST` if you're using PyOpenSSL: best to change both). I'm holding off on a more formal bit of documentation for now because ideally we'll land a 'custom SSL context' patch into urllib3 that will make this API way better."
- 2016-09-30, the same maintainer, issue 3608: "If you _absolutely must_, you can set `requests.packages.urllib.util.ssl_.DEFAULT_CIPHERS = 'RSA+3DES'`, and that will allow your connection to succeed. But I strongly discourage doing that."
- 2017-01-26, issue 3833: "As much as possible we don't want users to have to touch this code at all. That's why changing this code is tricky (and undocumented)."
- 2017-02-23, issue 1308: "*Please* do not do it this way. Instead, follow the approach in [this blog post](https://lukasa.co.uk/2017/02/Configuring_TLS_With_Requests/) and change the ciphers only for specific sites."

Later evidence: in 2026 a maintainer answered issue 6831 that "the packages module has always been an internal Requests detail ... anything under there is not intended to support mutation" (`../upstream/issue-6831-comments.json`).

## Reading

The written contract is the Transport Adapter with its own TLS context; this change broke it too, which is reference family GT-i1. Mutating the private urllib3 global after import is a practice a maintainer handed out in 2015, the maintainers steered users away from in 2017 and urllib3 removed in 2023. It can still fail only for programs pinned to urllib3 1.26.x.
