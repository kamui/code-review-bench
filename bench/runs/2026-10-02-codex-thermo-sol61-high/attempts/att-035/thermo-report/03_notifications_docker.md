# Notifications and Dockerfile

## Assessment

No actionable finding in these changes. Both refactors are proportionate to
their responsibilities. The shared title helper removes a real policy duplicate,
and the Dockerfile changes retain the order and contents of the operations.

Evidence was obtained with:

```sh
git diff main...review-head -- internal/notify.go docker/Dockerfile
nl -ba internal/notify.go
cat docker/Dockerfile
```

`notify.go` shrinks from 248 to 238 lines and increases from five to six
top-level functions. The Dockerfile shrinks from 23 to 22 lines. Neither
approaches the skill's file-size threshold.

## Notification title policy

Previously Telegram, ntfy, and Slack each contained the same switch selecting
one of three titles. The extracted helper at `internal/notify.go:31–40` retains
the exact comparisons and default branch. The title strings, including their
Unicode characters, are copied directly from the old literals. Telegram uses
the result as the start of its text, ntfy sets it in the Title header, and Slack
passes it to the same text message option.

| Counts | Previous and current title selection |
| --- | --- |
| `succeeded > 0`, `failed == 0` | Success |
| `succeeded > 0`, `failed > 0` | Completed with errors |
| `succeeded == 0`, `failed > 0` | Failure |
| `succeeded == 0`, `failed == 0` | Failure |
| Any other integer combination | Same default/conditions as before |

The last row includes negative counts: the refactor does not add validation or
change their fallback. No title normalization, new status enum, or revised
empty-run semantics is required to preserve this implementation's behavior.

This is a worked code-judo success already in the patch:

```go
text := backupStatusTitle(succeeded, failed)
req.Header.Set("Title", backupStatusTitle(succeeded, failed))
title := backupStatusTitle(succeeded, failed)
```

These are excerpts from three distinct callers, not one contiguous block. One
small pure function replaces three identical policy switches. The abstraction
earns its existence through those callers; it does not hide a generic plugin or
transport framework. Message bodies, error text, HTTP construction, notification
suppression, and transport delivery are outside the changed lines.

Verification status: exact branch and literal comparison by source inspection.
The package test passed as documented in [02_backup_tests.md](02_backup_tests.md),
but its mocked webhook tests do not call this helper. No direct Telegram, Slack,
or ntfy delivery was executed. No unverified transport success is claimed.

## Dockerfile changes

The separate nonroot account-creation and package-installation RUN instructions
become one chain. `addgroup` and `adduser` still run before `apk add`; each step
still stops subsequent execution when it fails because the chain uses `&&`.
Package order becomes alphabetical, and the set remains bash, ca-certificates,
curl, git, git-lfs, grep, and jq. The image user, work directory, binary handling,
permissions, and entrypoint remain unchanged.

Quoting the download URL makes the expanded release tag part of one shell
argument. For normal release tags the request URL is unchanged; whitespace or
wildcard characters can no longer cause shell word splitting or pathname
expansion. The tag is a build argument as before. This is a direct shell
boundary improvement, not additional control-flow machinery.

The quoted cache wildcard and existing curl/tar failure handling predate this
diff. They were not treated as newly introduced issues, and the review does
not expand into an unrelated image-hardening rewrite.

Verification status: static shell/Dockerfile comparison only. Docker is
unavailable and no image build or release download was attempted. A separate
generic installer abstraction would add indirection to this small sequence;
there is no further structural change required here.
