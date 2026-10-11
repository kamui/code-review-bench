# Notification titles and Dockerfile

## Notification source evidence

Reviewed `git diff main...review-head -- internal/notify.go`, full numbered
head source, and the base source. `notify.go` decreases from 248 lines to
238 while adding one function. `backupStatusTitle` at lines 31–40 is the
canonical owner of a three-way status-to-title rule formerly repeated in
Telegram, ntfy, and Slack senders. The three constants preserve the original
Unicode strings, including their existing variation-selector characters.

This is a good code-judo change: it deletes two complete duplicate switches
and lets each sender keep only its transport-specific formatting. It avoids
introducing a new status type or general notification framework for a rule
that is already adequately expressed by two counts.

The source-level behavior matrix is:

| Succeeded | Failed | Title |
| --- | --- | --- |
| Positive | Zero | `titleBackupsSucceeded` |
| Positive | Positive | `titleBackupsErrors` |
| Zero | Positive | `titleBackupsFailed` |
| Zero | Zero | `titleBackupsFailed` |
| Any other combination | Any other combination | The original switch's fallback when neither explicit case matches |

The final row includes defensive negative-count inputs; the helper retains
the exact switch conditions, so those behave as before too. No new empty
run policy is introduced. Telegram still appends completed/failed counts
and the first nonnil leading error; ntfy still sets the Title header; Slack
still passes the title as message text. Error extraction and transport
behavior outside the title selection are unchanged.

No actionable finding is raised for this subsystem. Further centralizing
the repeated count/body text would be a small optional cleanup and risks
obscuring differences such as Slack's multiple-error attachment. There is
no need to expand this successful pure helper into a transport abstraction.

The internal package compiles this helper and passed. No actual Telegram,
ntfy, or Slack request was sent. The webhook tests that ran cover a separate
notification path. Title equivalence is supported by direct comparison of
the three old switches with the new switch, not by a new runtime test.

## Docker source evidence

Reviewed `git diff main...review-head -- docker/Dockerfile` and the full head
Dockerfile. It decreases from 23 to 22 lines. Consecutive user/group setup
and apk installation now share one `RUN`, chained with `&&`; failure still
prevents subsequent shell steps and image completion. Sorting the package
list preserves its membership. The release-download URL now quotes `${TAG}`
inside the complete URL, preventing shell splitting and wildcard expansion.
For ordinary release tags its resulting argument is unchanged.

The base image, executable download path, extraction, removal, move,
permissions, runtime user, and entrypoint are unchanged. The package-cache
removal already used a quoted wildcard at the base, so its literal-glob
behavior is not a new defect introduced by this PR and is not a finding.

No actionable finding is raised for the Docker changes. A further
restructuring would not remove enough complexity to justify changing this
small flow. Docker is unavailable under the execution policy, so this
assessment is static: the image was not built and release downloads were
not attempted.
