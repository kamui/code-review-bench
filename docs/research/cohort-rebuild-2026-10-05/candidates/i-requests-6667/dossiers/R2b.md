# R2b: calling `truststore.inject_into_ssl()` after `import requests` makes every verified request raise RecursionError

Candidates: the "injecting another SSLContext implementation" part of NC-24be389c445e. Pull request: psf/requests #6667, base `8dd3b26b`, head `4089f3dc`.

## Problem

`truststore` is a library that makes Python check certificates against the operating system's trust store. An application turns it on with one call, `truststore.inject_into_ssl()`, which replaces Python's `ssl.SSLContext` class.

If that call comes after `import requests`:

- Before the change, it worked. Every later request used the operating system's trust store.
- After the change, every HTTPS request with the default `verify=True` raises `RecursionError: maximum recursion depth exceeded`.

Grouping: I split this out of the original group R2. The claim as raised said such an injection is "silently ignored". That is not what happens: it is a hard failure on every request. R2a covers the rest of R2 (changes to the default CA bundle path or file).

## What changed

```diff
+_preloaded_ssl_context = create_urllib3_context()
+_preloaded_ssl_context.load_verify_locations(
+    extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
+)
 ...
+    elif verify is True:
+        pool_kwargs["ssl_context"] = _preloaded_ssl_context
```

The first lines create a TLS context while `requests` is being imported. The last line reuses that one object for every verified connection.

A context created before the injection is an object of Python's original class. On each connection urllib3 sets a property on the context it is given (`context.verify_mode = ...`). Python's own code for that property looks up the name `ssl.SSLContext` at the moment it runs. After the injection that name is truststore's class, and the lookup loops back on itself until Python gives up:

```
urllib3/connection.py:768 in _ssl_wrap_socket_and_match_hostname: context.verify_mode = resolve_cert_reqs(cert_reqs)
ssl.py:738 in verify_mode: super(SSLContext, SSLContext).verify_mode.__set__(self, value)
ssl.py:738 in verify_mode: super(SSLContext, SSLContext).verify_mode.__set__(self, value)
... (repeats)
```

Before the change, requests held no context of its own. urllib3 built a fresh one for each pool, after the injection, from truststore's class.

## Intended or announced

Not intended and not announced. The pull request thread, the description and the 2.32.0 release note do not mention truststore or any replacement of the TLS implementation.

truststore's own guide shows the call placed before the networking imports:

```python
import truststore
truststore.inject_into_ssl()

# Automatically works with urllib3, requests, aiohttp, and more:
import urllib3
```

It does not say that the other order fails. It shipped in requests 2.32.0 (2024-05-20, a minor release) and stayed through 2.32.4.

## What the affected person sees

**Who:** authors and users of applications, scripts and command-line tools that import requests at the top of a file and call `truststore.inject_into_ssl()` later, for example in `main()`. These are typically corporate environments that need the operating system's trust store.

**After the change:** every HTTPS request made with the default `verify=True` fails with

```
RecursionError: maximum recursion depth exceeded while calling a Python object
```

from deep inside Python's `ssl` module. It is not a requests exception. Nothing in the message points at import order.

**How stuck:** requests with `verify=False` or `verify=<CA file>` still work, which does not help someone who wants the system store. The real fixes are: move the injection above `import requests`; pin `requests<2.32`; upgrade truststore to 0.10.1 or later, which works around it; or upgrade requests to 2.32.5.

**Before the change:** the same program worked.

## What the maintainers did

They disagreed with each other.

- A requests maintainer, answering a user who reported exactly this on 2.32.x (2024-05-23), said it is misuse and not a requests problem: "This is already known behavior that was reported to be broken in arcgis previously due to misuse of truststore ... It will need to be fixed there. That's unrelated to the topic of this issue. ... `truststore` MUST be imported before any networking code for either urllib3 or Requests." (https://github.com/psf/requests/issues/6715#issuecomment-2127176303)
- The author of truststore, who is also a member of the requests organisation on GitHub, answered a user who found that pinning requests 2.31.0 fixed it (2024-07-24): "Yeah this seems like a Requests bug, unfortunately. Going to close this issue." (https://github.com/sethmlarson/truststore/issues/143#issuecomment-2248570905)
- truststore then shipped a workaround in 0.10.1 that replaces requests' stored context during injection. Its source comment names this pull request: "requests starting with 2.32.0 added a preloaded SSL context to improve concurrent performance; this unfortunately leads to a RecursionError, which can be avoided by patching the preloaded SSL context with the truststore patched instance / also see https://github.com/psf/requests/pull/6667".
- requests itself never fixed this case directly. The revert (#6767, released in 2.32.5 on 2025-08-18) removed it together with the feature.

User reports: requests #6715 (comments), truststore #143, #165 ("Starting with the release 2.32.0 of the requests library the truststore library always leads to an recursion error") and #174.

## How each fact is known

- run: injection after import works at base and raises `RecursionError` at head; injection before import works at both; `verify=False` and `verify=<file>` avoid the error at head; truststore 0.10.4 avoids it at head (`probes/R2b/probe.py`, `result-base.txt`, `result-head.txt`).
- run: the traceback excerpt above (`result-head.txt`).
- run: release 2.32.4 fails the same way; 2.32.5 works (`result-release-*.txt`).
- read: the diff; the maintainer statements; truststore's guide, release notes and workaround commit; the membership of the truststore author in the requests organisation (`upstream/issue-6715-comments.json`, `upstream/ext-truststore-*`, `upstream/sethmlarson-association-comment.json`).
- reported: the users' accounts in the four issues above. One of them says it failed only on Windows for them; here it fails on Linux.

Not run: Windows and macOS, where truststore uses the platform's own verification; `urllib3.contrib.pyopenssl.inject_into_urllib3()`, which the claim also names.

## Relation to existing reference bugs and ruled claims

This is new. It is not a manifestation of GT-i1 to GT-i4.

- GT-i1 is about an adapter's own context being replaced. No adapter is involved here.
- GT-i2 is about work that `import requests` performs and can fail on. Here the import succeeds; the failure comes on each request. The two share a cause (the context is created at import), and a fix that creates the context on first use would cure the common form of this problem. It would not cure an injection made after the first request.
- GT-i3 and GT-i4 concern CA sources and client certificates.

There are no ruled claims for this pull request.

## Both sides

For calling it a bug:

- It worked before the change and fails on every request after it, with a reproduction at both commits.
- Several independent users reported it. A downstream library had to ship a workaround that names this pull request.
- A member of the requests organisation called it a requests bug.
- The error gives the user nothing to go on.

Against:

- A requests maintainer said plainly that this order of calls is misuse of truststore and not requests' concern.
- truststore's guide shows the injection first. Injecting after other code has created TLS contexts was already fragile before this change; requests had simply not created one until now.
- The remedy on the user's side is one moved line.

## Recommendation

`eligible`, as a new bug. Confidence: medium.

Strongest argument against: the requests maintainer who looked at this exact report ruled that the trigger is unsupported use of another library. If the owner accepts that, the right label is `advisory`.
