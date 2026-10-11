# Notification status and Dockerfile changes

## Notification evidence

`git diff main...review-head -- internal/notify.go` shows 18 additions and 28 deletions. The file shrinks from 248 to 238 lines, measured with `git show main:internal/notify.go | wc -l` and `wc -l internal/notify.go`. The change adds three status constants at head lines 26–28 and a shared `backupStatusTitle` at 31–40. Its consumers are Telegram at 104, ntfy at 191, and Slack at 216.

All three removed switches used the same conditions and literal strings. The new helper returns success when `succeeded > 0 && failed == 0`, mixed errors when both are positive, and failure otherwise. The zero/zero case still returns the failure title, and unusual negative counts retain the former default behavior. Emoji and variation-selector bytes are copied in the committed diff, including the leading selector in the mixed-error and failed literals. The title computation moves to the same semantic point in each consumer, and request/message construction is otherwise unchanged.

This abstraction earns its keep. One pure policy helper replaces three switches, gives the strings canonical ownership, and avoids introducing another state type or channel-specific branch. There is no actionable notification finding. Broad redesign of notification transport, shared clients, or error handling would address preexisting code outside this diff and is not necessary for this extraction.

## Dockerfile evidence

`git diff main...review-head -- docker/Dockerfile` shows three additions and four deletions. The first two `RUN` instructions are combined by joining the group/user creation and `apk add` commands with `&&`. The old package list and the new list both contain exactly `bash`, `ca-certificates`, `curl`, `git`, `git-lfs`, `grep`, and `jq`; only their order changes. Installation flags and the subsequent cache-removal command remain unchanged. Group and user creation still precede package installation, and both occur before `USER nonroot`.

The release-download URL at head line 13 is now quoted as one shell argument. `${TAG}` remains expanded inside the double quotes, and the HTTPS protocol restriction, minimum TLS version, output filename, extraction, cleanup, move, and permission commands remain unchanged. Quoting prevents whitespace or glob characters in a tag from causing shell word splitting, which is an intended improvement rather than a maintainability regression.

The preexisting quoted wildcard in `rm -f "/var/cache/apk/*"` is unchanged, and `apk --no-cache` is retained. It is not reported as a newly introduced cache-removal bug. The base image, working directory, runtime user, and entrypoint are unchanged. There is no actionable Dockerfile finding in the pinned changes.

## Verification and limits

The internal suite passed with the prescribed offline Go environment. It compiles the notification change, but the existing test files do not directly exercise every notification title branch through every transport. Equivalence is established here by comparing the three removed switches with the extracted switch and their consumers. This is source verification, not a claim that real Telegram, ntfy, or Slack calls were executed.

Docker is unavailable under the execution policy, so no image build or container run was attempted. Package identity, shell argument quoting, and command chaining were verified from the committed source. No network documentation, upstream review, or external reference answer was consulted. No production remedies were applied.
