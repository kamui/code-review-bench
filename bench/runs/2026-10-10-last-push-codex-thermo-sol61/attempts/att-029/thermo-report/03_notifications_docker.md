# Notifications and Docker

## Scope and measurements

Reviewed `git diff main...review-head -- internal/notify.go docker/Dockerfile`
alongside the complete head files. `notify.go` changes by +18/−28 and
shrinks from 248 to 238 lines. The Dockerfile changes by +3/−4 and
shrinks from 23 to 22 lines. Neither file approaches the skill's file-size
threshold. There are no additional actionable findings in these changes.

## Notification status

The base has three identical switches for Telegram text, the ntfy title
header, and Slack message text. The head extracts their exact string
literals into constants at `internal/notify.go:26–28` and their decision
into `backupStatusTitle` at lines 31–40. The helper is reused at lines
104, 191, and 216. It owns one actual shared decision; it is not a thin
identity wrapper.

| Succeeded | Failed | Result in base and head |
| --- | --- | --- |
| Positive | Zero | `titleBackupsSucceeded` |
| Positive | Positive | `titleBackupsErrors` |
| Zero | Positive | `titleBackupsFailed` |
| Zero | Zero | `titleBackupsFailed` |
| Any other combination | Any | Default failure unless one of the first two guards matches |

The comparison operators and literal bytes are identical in the diff,
including the existing Unicode variation characters in the error and
failure strings. The zero/zero default is retained. Message statistics,
error descriptions, URL construction, Slack attachments, and HTTP
execution are unchanged by this extraction.

This is the clearest code-judo move in the PR: it deletes duplicate
branches by giving the shared concept one canonical implementation.
Replacing the two integers with a new status enum or merging the channel
senders would introduce concepts and touch transport-specific behavior
without an evident benefit for this change. No broader notification
framework is warranted.

The two selected webhook format tests pass. They exercise webhook
serialization and do not call `backupStatusTitle`, so they are not direct
verification of the new helper. Its equivalence was checked statically
against all three old switches. No Telegram, ntfy, or Slack service was
contacted, and no new test was added to the checkout.

## Docker command changes

`docker/Dockerfile:5–8` combines nonroot group/user creation and package
installation into one `RUN`, retaining command order and `&&` failure
propagation. All seven packages remain: bash, ca-certificates, curl, git,
git-lfs, grep, and jq. Sorting them changes presentation rather than the
requested package set. The later `USER nonroot` instruction is unchanged,
so installation still runs under the same user context as before.

At line 13 the release URL is quoted as a single shell argument. This
retains the expected URL for ordinary tags while preventing word
splitting or glob expansion in unusual values. `ARG TAG`, download
options, archive extraction, binary movement, permissions, and entrypoint
are unchanged.

The quoted `"/var/cache/apk/*"` cleanup pattern is inherited from the
base. It is not a PR-introduced glob problem and is not an actionable
finding here. The `--no-cache` package option also remains in place.
Likewise, release-download integrity and version pinning are outside the
changed behavior and were not promoted into speculative findings.

There is no obvious structural improvement beyond the current command
consolidation. A multi-stage rebuild or download framework would change
the Docker delivery approach rather than simplify these small edits.

Docker is unavailable under the execution policy. No image build or
release download was attempted. Static inspection establishes unchanged
package membership and command sequencing, not runtime availability of
the Alpine repositories or release artifact.

## Verification record

`git diff main...review-head --check` completed with exit status 0. The
focused offline package tests also passed as recorded in `01_backup.md`.
These results do not substantiate the PR author's full-suite, build,
vet, or lint claims; those were not rerun in this review. The checkout
remained clean at the pinned head.
