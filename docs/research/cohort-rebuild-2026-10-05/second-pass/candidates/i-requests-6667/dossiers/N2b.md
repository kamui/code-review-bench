# N2b: enabling TLS key logging after import no longer records default verified connections

## Problem

A program sets `os.environ["SSLKEYLOGFILE"]` after importing Requests but before its first HTTPS request. At base the connection writes the keys needed to decrypt a captured TLS conversation for diagnosis. At head a default verified connection writes no keys. The request itself still succeeds.

This is split from N2's general statement about later defaults. Cipher choice, N2a, affects connections and TLS policy. N2b affects diagnostic output. The original statement did not name this environment variable. This is a tested instance of its general claim, not extra original wording supplied for recovery credit.

## What changed

Read, in `src/requests/adapters.py`:

```diff
+_preloaded_ssl_context = create_urllib3_context()
...
+    elif verify is True:
+        pool_kwargs["ssl_context"] = _preloaded_ssl_context
```

Read, urllib3 1.26.18's context factory reads `SSLKEYLOGFILE` and assigns `context.keylog_filename`. At base it reads the environment on connection creation. At head it reads during import. Updating the environment later cannot update the existing context.

Run, Python 3.10.12 and urllib3 1.26.18, using fresh processes and a local TLS-1.2 server:

| When enabled | Base | Head |
| --- | --- | --- |
| After import, `verify=True` | 200 and `CLIENT_RANDOM` keys | 200, no keys |
| After import, `verify=<CA file>` | 200 and keys | 200 and keys |
| Before import, `verify=True` | 200 and keys | 200 and keys |

The saved head context has `keylog_filename=None` in the failing case. Tests inspect actual output for handshake keys, not merely whether a file exists. Certificates and transient key files were created only in scratch.

## Intended or announced

Read, the PR announces root-certificate caching, with no documentation change and no mention of key logging. The feature shipped in Requests 2.32.0 on 2024-05-20; its release note describes global-context reuse. The 2.32.5 source removes that context.

Read, urllib3 1.26.18 documents key logging to decrypt captured traffic. Its example is a shell command, `export SSLKEYLOGFILE=/path/to/keylogfile.txt`, before starting the application. That form still works. It does not promise that setting the environment from code after a networking library import works.

## What the affected person sees

Run, the HTTP response is 200 at both commits. The late-enabled diagnostic file has no handshake keys at head. Someone diagnosing encrypted traffic cannot decode that connection from this file. There is no exception explaining why.

Run, enabling logging before import or passing an explicit CA file restores logging. Read, no packet or fetched pre-merge program example shows users relying on this late placement. The scope of the established loss is a synthetic diagnostic sequence, rather than a failed ordinary application or a missing required audit record.

## What the maintainers did

Read, neither the fetched PR discussion nor its review records name this loss. The Requests and urllib3 searches found key-logging discussion but no acknowledgement of the late-setting regression.

Read, older Requests issue [3674](https://github.com/psf/requests/issues/3674) asks for session-key logging for Wireshark. Its historical answers concern support in OpenSSL and Python before those bindings existed. They are not a judgment on this change.

Read, the general revert in [6767](https://github.com/psf/requests/pull/6767) removed the stored context. Requests 2.32.5 shipped it on 2025-08-18. No explicit exact-problem acknowledgement was found. The revert and release notes confirm what changed later; they do not show that late logging was an expected use before merge.

Raw responses are in `../upstream/urllib3-advanced-1.26.18.json`, `urllib3-ssl-1.26.18.json`, `issue-3674.json`, `issue-3674-comments.json`, `search-keylog.json`, `search-late-keylog.json`, `pr-6667*.json`, `issue-6667-comments.json`, `pr-6767*.json`, `releases.json`, and both Requests release-tag source responses.

## How each fact is known

- Run: real key-file contents, successful requests and both restoring controls. See `../probes/N2b/probe.py`, `result-base.txt`, `result-head.txt`, `environment.txt`. That entry point runs the saved suite in `../probes/N2a/probe.py`, using the TLS helper in `../probes/N1/probe.py`.
- Read: pinned diff, urllib3 factory and documentation, pre-merge discussion, older logging issue, release notes, release-tag source and revert.
- Reported: the older issue author's desire to analyze traffic with Wireshark. No user's late-setting regression establishes reachability here.

The code, logging documentation and probe behavior were knowable before merge. Releases and revert are later evidence. Not run: Wireshark decryption, a real user's diagnostic workflow, packaged release binaries, Windows or macOS.

## Relation to existing reference families and ruled claims

This is not GT-i6's replacement TLS implementation or GT-i8's ignored version restrictions. It is not GT-i2's import cost or import failure. Moving construction to first use would repair this first-request example, but still miss logging enabled later. N2a shares creation-time capture but concerns a different operation. There is no first-round ruling on this logging case. Ruling 30's advisory treatment of an undocumented late default-bundle override is a close precedent.

## Both sides

For eligibility: urllib3 provides a concrete logging feature. The same application sequence loses diagnostic output after the change, with a base/head run. Someone investigating a difficult network failure can lose useful information.

Against eligibility: the documented setup sets the variable before process startup and still works. Nothing establishes late setting as documented, deliberately supported or a practice users actually follow. No failed decoding workflow or material loss beyond this diagnostic experiment is shown. The unusual-input rule therefore points below the threshold.

## Recommendation

`advisory`, confidence medium. Explain that settings read during context creation must precede Requests import. Propose no impact band.

Strongest argument against: late logging is a natural way to turn on diagnosis inside an existing script, and silently losing the output can impede that task. Evidence that people relied on this placement before merge would strengthen an eligible diagnostic-loss claim.
