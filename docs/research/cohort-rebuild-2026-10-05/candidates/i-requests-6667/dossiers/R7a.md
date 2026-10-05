# R7a: an adapter's host-name settings permanently change the TLS context that every session shares

Candidate: the `assert_hostname` / `assert_fingerprint` part of NC-27f6bd8ffa63. Pull request: psf/requests #6667, base `8dd3b26b`, head `4089f3dc`.

## Problem

Some adapters tell urllib3 to check the server's certificate in their own way: `assert_hostname` (expect this name instead of the URL's) or `assert_fingerprint` (expect exactly this certificate). The requests-toolbelt package ships adapters that do this.

After the change, the first `verify=True` request through such an adapter switches off OpenSSL's host-name checking on the one TLS context that every session in the process shares. It stays off for the life of the process.

On its own this changes nothing a user can observe: urllib3 notices the setting is off and checks host names itself. Its importance is what it enables. With host-name checking off, the request described in R1 (a plain `http://` URL through an `https://` proxy) no longer raises an error. It silently sets the shared context to "do not verify", and while that lasts, ordinary `verify=True` requests in other threads accept certificates they must refuse.

Grouping: I split the original group R7 in two. R7a is this problem. R7b (CA material from an adapter) is the same problem as R3. R7a and R1 are two halves of one fault and I recommend ruling them together.

## What changed

```diff
+    elif verify is True:
+        pool_kwargs["ssl_context"] = _preloaded_ssl_context
```

Every `verify=True` pool now receives the same context object. When urllib3 connects, it writes that connection's verification settings into the context it was given:

- it sets the context's verification mode from the pool's `cert_reqs`;
- if the pool has `assert_hostname` or `assert_fingerprint`, it sets `check_hostname = False` on the context, because it will do that check itself.

Before the change each pool had a private context, so these writes touched nothing else. Now they land on the shared object. `check_hostname` is never set back.

Python refuses to set verification mode "none" on a context whose `check_hostname` is on. That refusal is the `ValueError` of R1. Once an adapter has switched `check_hostname` off, the refusal is gone, and R1's request succeeds in writing "none" into the shared context. A verified request in another thread that is between "set my mode" and "handshake", or between "handshake" and "check the host name", then proceeds without the check.

With urllib3 1.26.x the shared context starts with `check_hostname` off, so no adapter is needed; that case is in the R1 dossier.

## Intended or announced

Not intended and not announced. The thread shows the reviewers knew a shared context must not be reconfigured, and assumed only users would do so:

- A CPython core developer: "It is thread safe as long as you don't reconfigure it once it is used by a connection. Adding new certs to the internal trust store is fine, but changing ciphers, verification settings, or mTLS certs can lead to surprising behavior. The problem is unrelated to threads and can even occur in a single-threaded program." (https://github.com/psf/requests/pull/6667#issuecomment-2094634639)
- A maintainer, asking for the context to be renamed private: "That will encourage people to modify it in a way we don't want to be supporting." (https://github.com/psf/requests/pull/6667#issuecomment-2094840318)
- The approving maintainer: "I was curious if we'd be better off doing this per-Adapter instance instead of globally but it seems like that may not be a concern with Christian's response."

Nobody noted that urllib3 itself rewrites verification settings on every connection. It shipped in 2.32.0 (2024-05-20, a minor release) and stayed through 2.32.4.

## What the affected person sees

**Who:** people running a multi-threaded process that (1) uses a pinning adapter with `verify=True` somewhere, and (2) sends plain-HTTP requests through an HTTPS proxy somewhere, on urllib3 2.x. The person harmed is whoever makes ordinary verified requests in that process; they have done nothing unusual.

**What they see:** nothing. No error and no warning. In a 20-second run with three threads sending proxied `http://` requests and six threads making ordinary `verify=True` requests to two servers that must be refused, after one request through a pinning adapter:

| | certificate for the wrong host name | certificate from an untrusted CA |
| --- | --- | --- |
| before the change (`assert_hostname` adapter) | 0 of 482 accepted | 0 of 485 accepted |
| after the change (`assert_hostname` adapter) | 781 of 3346 accepted | 251 of 3624 accepted |
| after the change (`assert_fingerprint` adapter) | 630 of 3013 accepted | 193 of 3273 accepted |
| release 2.32.4 (`assert_hostname` adapter) | 718 of 3286 accepted | 226 of 3551 accepted |
| release 2.32.5 | 0 of 471 accepted | 0 of 472 accepted |

**Without the proxied requests:** the only visible difference is the wording of the error for a wrong host name. Before the adapter is used it is OpenSSL's `certificate verify failed: Hostname mismatch`; afterwards it is urllib3's `hostname 'localhost' doesn't match 'other.example'`. The connection is refused either way.

**In a single thread:** no certificate is wrongly accepted. Each verified connection sets the mode back before it connects.

**Before the change:** none of this could happen; there was no shared context.

## What the maintainers did

- No maintainer named this problem.
- While fixing GT-i1 a maintainer stated a wider intent that would have covered it: "we're currently looking at disabling the SSLContext optimization in the event we have a PoolManager with any custom configuration kwargs." (https://github.com/psf/requests/pull/6716#issuecomment-2128428673). The fix that shipped in 2.32.3 only looks for an adapter `ssl_context`. Release 2.32.4 still behaves as the head does.
- The revert (#6767, merged 2025-06-13, released in 2.32.5 on 2025-08-18) removed the shared context. Its description speaks of "the number of edge cases and concurrency issues we've encountered with this change" without listing them. The release note says the feature "has created a new class of issues in Requests".
- The repository's security advisories do not mention it.

## How each fact is known

- run: after one `verify=True` request through an adapter with `assert_hostname` or `assert_fingerprint`, the shared context reads `check_hostname=False` at head; no shared context exists at base (`probes/R7a/probe.py`, lines 1.0 and 1.4).
- run: in a single thread a plain session still refuses the wrong-host and untrusted servers afterwards; only the error text changes (lines 1.5, 1.6, 2.3, 2.4).
- run: after the flip, R1's proxied request returns 200 instead of raising, and leaves the shared context at `verify_mode=CERT_NONE` (lines 2.1, 2.2).
- run: the concurrency counts in the table (lines 3.1 to 3.3; `result-base.txt`, `result-head.txt`, `result-release-*.txt`). This is real thread timing with the interpreter's default settings; the counts change from run to run.
- read: the diff; the statements in the pull request thread and on #6716; #6767; the advisory list; the requests-toolbelt adapters that pass `assert_fingerprint` and `assert_hostname` (`upstream/field-requests-toolbelt-*`).
- reported: nothing. I found no user report of this.

Not run: a real network attacker; Python builds without the global interpreter lock; the requests-toolbelt adapters themselves (the probe uses a minimal adapter that passes the same setting).

## Relation to existing reference bugs and ruled claims

It has the same root as GT-i4: one context is shared, and urllib3 rewrites it for each connection. It is not the same bug.

- GT-i4's obligation is about a client certificate supplied with `cert=`. Here the settings are the verification mode and host-name checking, and what is lost is server verification in other threads.
- A fix that meets GT-i4's wording ("skipping the shared context when a certificate is supplied") leaves this in place.
- GT-i1 is about an adapter's own settings being lost. Here the adapter's setting works for the adapter and spills onto everyone else.
- GT-i2 and GT-i3 are unrelated.

Relation to the other candidates: R1 is the request that writes "do not verify" into the shared context. R7a is what removes the guard that otherwise stops that write on urllib3 2.x. One remedy fixes both: do not give the shared context to a pool whose verification settings differ from the default.

There are no ruled claims for this pull request.

## Both sides

For calling it a bug, together with R1:

- A reproduction at both commits shows verified requests accepting certificates for the wrong host and from an untrusted CA.
- Each ingredient is supported use: a published pinning adapter, a proxy setting, threads.
- The failure is silent and is the protection `verify=True` exists to give.
- The CPython developer's warning in the thread describes exactly this ("changing ... verification settings").

Against:

- Alone, it has no effect a user can observe. Host names are still checked.
- The harm needs three things at once: a pinning adapter used with `verify=True`, proxied plain-HTTP traffic, and concurrency.
- No user reported it and no maintainer named it.
- If R1 is accepted as the bug "an unverified connection is given the shared verified context", the harm shown here can be counted there, with R7a as a precondition and not a separate bug.

## Recommendation

`eligible`, ruled together with R1 as one new bug: connections write their own verification settings into the context that all sessions share. Confidence: high on the facts; medium-low that the owner will want R7a counted as more than a note under R1.

Strongest argument against: by itself this is a state change with no failing behaviour, which is the definition of `advisory`. If the owner rules R7a apart from R1, `advisory` is the right label for R7a and the certificate-acceptance result belongs to R1.
