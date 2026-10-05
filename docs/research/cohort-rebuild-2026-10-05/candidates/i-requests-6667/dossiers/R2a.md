# R2a: the default CA bundle is read once at import; later changes to it are ignored

Candidates: NC-799dd6f6538a, and the bundle-path and bundle-file parts of NC-24be389c445e. Pull request: psf/requests #6667, base `8dd3b26b`, head `4089f3dc`.

## Problem

Before the change, requests looked up its default CA bundle every time it prepared a verified connection. After the change it loads the bundle once, when `requests` is imported. Three things done after import therefore stop having an effect on `verify=True` requests:

1. Reassigning `requests.adapters.DEFAULT_CA_BUNDLE_PATH` to another file.
2. Rewriting the bundle file in place (for example a certifi upgrade under a running process).
3. Deleting the bundle file. The old error for a missing bundle no longer appears.

Grouping: I split the original group R2 in two. This dossier, R2a, is about the bundle path and file. R2b is about swapping the TLS implementation after import (`truststore.inject_into_ssl()`), which fails in a different and much louder way.

Two parts of the claim as raised are wrong. Reassigning `requests.utils.DEFAULT_CA_BUNDLE_PATH`, or patching `requests.certs.where` / `certifi.where` after import, never worked, before or after the change.

## What changed

```diff
+_preloaded_ssl_context = create_urllib3_context()
+_preloaded_ssl_context.load_verify_locations(
+    extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
+)
```

This runs once at import. And in `cert_verify`, the per-request lookup is removed:

```diff
-            if not cert_loc:
-                cert_loc = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
-
-            if not cert_loc or not os.path.exists(cert_loc):
-                raise OSError(
-                    f"Could not find a suitable TLS CA certificate bundle, "
-                    f"invalid path: {cert_loc}"
-                )
```

Before, `cert_verify` read the module's `DEFAULT_CA_BUNDLE_PATH` name on each request, checked the file existed, and told urllib3 to load that file for each new connection. Now the name is read once and the certificates are held in memory for the life of the process.

## Intended or announced

Loading once is the purpose of the change. The description says: "It isn't possible to skip loading root CA certificates entirely, but it isn't necessary to do it on every request." The release note for 2.32.0 says "`verify=True` now reuses a global SSLContext".

Nobody discussed what happens to code that changes the default after import. Nothing was deprecated. requests documents `DEFAULT_CA_BUNDLE_PATH` only as a value to read ("By default the list of certificates trusted by Requests can be found with: `from requests.utils import DEFAULT_CA_BUNDLE_PATH`"). The documented ways to use another bundle are `verify=<path>`, `Session.verify` and the `REQUESTS_CA_BUNDLE` environment variable. All three still work after the change.

It shipped in 2.32.0 (2024-05-20, a minor release) and stayed through 2.32.4.

## What the affected person sees

**Who:** people running programs that set requests' default bundle by assigning to the module global after importing requests. This is not a documented interface, but public code does it. Three examples found by code search:

- A NETWAYS monitoring plugin, to honour a CA file the operator passes in: `requests.adapters.DEFAULT_CA_BUNDLE_PATH = cafile_path`.
- BleachBit and the Datadog agent (v5), to point a frozen Windows build at the `cacert.pem` shipped beside the executable.

**After the change:** the assignment is silently ignored. If the server's certificate comes from a private CA in the assigned file, every request fails with

```
SSLError(SSLCertVerificationError(1, '[SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: unable to get local issuer certificate'))
```

The error looks like a wrong CA file, not like a library change. The person running the program can work around it without touching code by setting `REQUESTS_CA_BUNDLE`. The author has to switch to `verify=` or `Session.verify`.

Where the assigned file holds the same public roots as certifi (the frozen-app case), nothing visible changes.

**Rewritten file:** a process keeps trusting the certificates it loaded at import until it restarts. Before, new connections picked up the new file.

**Deleted file:** before, the next verified request raised `OSError: Could not find a suitable TLS CA certificate bundle, invalid path: ...`. After, the process carries on with the certificates it already has in memory. No one is worse off.

## What the maintainers did

- No maintainer said anything about changing the default after import, in the pull request or later.
- The revert (#6767, merged 2025-06-13, released in 2.32.5 on 2025-08-18) restored the per-request lookup. In 2.32.5 the reassignment and the rewritten file are honoured again and the missing-file error is back.
- Related but different: issue #6749 and pull request #6781 are about `import requests` failing when the default bundle file does not exist. That is the import-time loading already recorded as GT-i2.

## How each fact is known

- run: reassigning `requests.adapters.DEFAULT_CA_BUNDLE_PATH` after import is honoured at base and ignored at head (`probes/R2a/probe.py`, case `rebind-adapters`).
- run: reassigning `requests.utils.DEFAULT_CA_BUNDLE_PATH`, and patching `requests.certs.where` / `certifi.where` after import, are ignored at base and at head (cases `rebind-utils`, `patch-certs-where`).
- run: a rewritten bundle file is picked up at base and not at head; a deleted file raises the `OSError` at base and nothing at head (cases `rewrite-file`, `delete-file`).
- run: `REQUESTS_CA_BUNDLE` and `verify=<path>` work at both commits.
- run: releases 2.32.4 (same as head) and 2.32.5 (same as base).
- read: the diff, the documentation line, the pull request thread, #6767, and the three public code examples (`upstream/field-*`, `upstream/codesearch-rebind-*.json`).
- reported: nothing. I found no upstream issue from a user hit by this.

Not run: any of the three public programs themselves.

## Relation to existing reference bugs and ruled claims

This is new in the sense that none of GT-i1 to GT-i4 describes it. It comes from the same lines as GT-i2 (the bundle is loaded at import instead of per request), but GT-i2's obligation is about what `import requests` is allowed to do and to fail on. A fix that satisfies GT-i2 by loading on first use and caching would leave this behaviour unchanged. It is not GT-i3 either: GT-i3 is about connections that do not receive the shared context at all.

There are no ruled claims for this pull request.

## Both sides

For calling it a bug:

- It is a behaviour change with a reproduction at both commits.
- Public code relies on the old behaviour, and for the private-CA case the result is a hard failure.
- Nothing announced it. The revert restored it.

Against:

- The trigger is monkeypatching a module global. requests never documented that as a way to configure trust, and every documented way still works.
- Half of the claimed triggers never worked.
- Caching the bundle is the whole point of the change, so "the bundle is no longer re-read" is a description of the feature.
- No user reported it in fifteen months of the 2.32 line.
- The long-running-process and deleted-file cases have no one who is clearly worse off.

## Recommendation

`advisory`. Confidence: medium.

The observation is correct and worth a reviewer's sentence, but the only failing case depends on an undocumented override while the documented ones keep working.

Strongest argument against: real programs did assign to `requests.adapters.DEFAULT_CA_BUNDLE_PATH`, it worked in every release up to 2.31.0, and after the change an operator who supplies a private CA to such a program gets certificate failures with no hint of the cause.
