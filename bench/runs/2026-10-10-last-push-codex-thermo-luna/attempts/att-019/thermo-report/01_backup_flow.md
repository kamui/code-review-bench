# Backup flow, startup configuration, and tests

## Assessment

No actionable finding. `internal/backup.go` grows from 729 to 774 lines, remaining well below the 1,000-line threshold. `internal/backup_test.go` ends at 989 lines and is shorter than its base version. The changed logic decomposes the named complex flows rather than adding a new conditional layer: `runProviderBackups` delegates provider collection; `displayStartupConfig` delegates per-provider output while sharing repeated formatting; `Run` sequences request-timeout logging, validation, work-directory creation, and scheduling; and `checkProvider` separates token-only from paired-credential validation.

## Evidence and code-judo review

In `internal/backup.go`, `collectProviderBackupResults` preserves the prior provider ordering and the same credential checks. Bitbucket retains its two complete-credential alternatives, and the other providers retain their individual token gates. The small local function table removes repeated execution branches while keeping the provider list and each associated backup function visible together. A more generic provider metadata framework would have to represent Bitbucket's two auth modes and provider-specific startup exceptions, adding machinery without clearly deleting concepts.

The startup logging helpers share the repeated organization, retention, compare-mode, and LFS formatting. Per-provider functions retain the provider-specific conditions and GitLab minimum-access default. This is a reasonable boundary: flattening these distinctions into a data-driven logger would move the exceptions into callbacks or flags and make the behavior less direct.

The scheduler branches now share job registration, start, and shutdown waiting through `runScheduledJob`; interval and cron definitions and options remain at their call sites. The helper accepts the scheduler's native job definition/options, avoiding a custom scheduling model. The split does not create extra scheduling modes or state.

The Gitea organization test extracts cohesive assertions and marks helpers with `t.Helper()`. The duplicate live GitLab test was removed. No new production branch or test-only behavior was identified as a maintainability regression.

## Verification status

- `go test ./internal/ -count=1` passed using `GOMODCACHE=<clone-cache>/gomodcache`, `GOCACHE=<clone-cache>/gocache`, `GOFLAGS=-mod=mod`, `GOPROXY=off`, and `GOTOOLCHAIN=local`.
- Static line counts: `internal/backup.go` 774; `internal/backup_test.go` 989.
- The package test pass verifies the available offline unit/package tests. Provider-credential live backups were unavailable and are skipped under the execution policy.

## Worked code-judo proposal and remediation

No change is proposed. The strongest apparent alternative was replacing startup output and backup dispatch with a generic provider registry. Reviewing the complete changed flow shows that the registry would need special cases for Bitbucket's alternative credential forms, GitLab's default access level, and GitHub's skip-user-repositories setting. That approach would relocate rather than remove the conditions. The current helpers keep each exception next to its provider and centralize only genuinely repeated behavior.
