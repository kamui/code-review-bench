# Notifications and container build instructions

## Assessment

No actionable finding. `internal/notify.go` now shares the three backup status titles through constants and `backupStatusTitle`, removing identical switches from Telegram, ntfy, and Slack. The helper preserves the same outcome mapping: success when at least one succeeds and none fail, partial-error when both counts are positive, and failed for the remaining cases. The Dockerfile consolidates adjacent setup `RUN` instructions, sorts apk package names, and quotes the interpolated release URL.

## Evidence and code-judo review

The notification helper is a small pure mapping with a clear three-way policy and a single canonical set of title strings. It removes duplication without adding another configuration surface or changing each transport's message formatting.

The Dockerfile edit keeps setup commands in their existing sequence within one layer. Quoting the URL makes the intended shell argument boundary explicit. Static review found no new branching, abstraction, or ownership-boundary concern.

## Verification status

- `git diff --check main...review-head` passed.
- The internal Go package tests passed (see `01_backup_flow.md`).
- Docker was unavailable under the execution policy, so image construction and Dockerfile linting were not run; this portion was reviewed statically.

## Worked code-judo proposal and remediation

No further restructuring is warranted. The shared title function already deletes the repeated decision logic; folding notification delivery into one cross-transport abstraction would couple independent APIs and is not supported by this diff. The Dockerfile change already combines the adjacent setup operations without mixing in release download behavior.
