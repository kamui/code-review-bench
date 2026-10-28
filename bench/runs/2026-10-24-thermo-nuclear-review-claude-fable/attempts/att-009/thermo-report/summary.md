# Thermo Nuclear Review — jonhadfield/soba#195 "refactor: resolve SonarQube findings"

Range audited: `c77f548cbd340d2e744da7c6d92a54372b4b900b..136a4850df9aec8cf8813b27ba1533d4a17bc642` (`git diff main...review-head`), 4 files, +312 / −305.

## Verdict

No actionable findings.

The pull request is a pure structural refactor. Every extracted helper was compared line by line against the code it replaced, and the observable behaviour (control flow, return values, error text, log text, provider ordering, scheduler options, notification titles) is preserved. There are no security, breaking-functionality, devex or feature-gate concerns to report.

## Verification performed

- `go build ./...`, `go vet ./...` and `go test ./internal/ -count=1` all pass offline on the head (live provider tests skip for lack of credentials). The clone tree was confirmed unchanged afterwards.
- `docker/Dockerfile` could not be built (no Docker), so it was reviewed by inspection only.

## What was traced, file by file

### `docker/Dockerfile`

Two consecutive `RUN` layers are merged, the apk package list is reordered alphabetically with no package added or removed, and `${TAG}` is now inside a quoted URL. Neither the quoting nor the merge changes what the shell executes. The pre-existing `rm -f "/var/cache/apk/*"` no-op is untouched and was not introduced here.

### `internal/notify.go`

The three title strings are hoisted into constants and selected by a new `backupStatusTitle(succeeded, failed)` helper. A byte-level comparison of the old inline literals and the new constants shows they are identical, including the stray U+FE0F variation selectors that precede the warning and alarm emoji. The `switch` conditions and default arm are the same as all three original copies, so Telegram, ntfy and Slack titles are unchanged.

### `internal/backup.go`

- `collectProviderBackupResults`: provider order (Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut), the per-provider non-empty-token gate, and the Bitbucket "API token OR OAuth complete" gate match the old inline block. The only delta is that the five Bitbucket env/file lookups are now short-circuited (`bitbucketAPITokenDefined() || bitbucketOAuthDefined()`), so when the API-token pair is complete the three OAuth variables are no longer read. The only side effect of a `GetEnvOrFile` read is a diagnostic log line when a `*_FILE` path is missing, so this can only remove a stray warning; it cannot change which providers run.
- `displayStartupConfig` and the per-provider `display*StartupConfig` helpers: for each of GitHub, Gitea, GitLab, BitBucket and Azure DevOps the set of lines, their order, their labels (including the pre-existing "Gitlab backup LFS" casing and the "Organistations" spelling) and the gating env var are identical to the old nested blocks. `compareTypeRefs`/`compareTypeClone` are `"refs"`/`"clone"`, so the `Printf("%s compare method: %s")` form emits the same text the old `Print` calls did. One comparison changed from `strings.ToLower(compare) == compareTypeRefs` to `strings.EqualFold(compare, compareTypeRefs)` for BitBucket. That affects only this startup log line, aligns BitBucket with the four other providers, and the actual compare method used by the backup is passed through separately from `os.Getenv(envBitBucketCompare)` in `bitbucket.go`, which is not touched.
- `checkProvider` / `checkJustTokenProvider` / `checkUserAndPasswordProvider`: the counting, the "parameter … is not defined" messages, and the aggregated error return are unchanged. The old code could add both counts when a provider appeared in both lists; the new code does the same via `count +=`.
- `checkProvidersDefined`: `bitbucketAPITokenComplete` is now computed once before the map iteration instead of being set inside the `providerNameBitBucketAPIToken` case. In the old code, because map iteration order is random, hitting the OAuth case first could count both Bitbucket methods; the new code counts at most one deterministically. `count` is only ever compared to zero, so the "no providers defined" outcome is identical in every environment.
- `Run` → `logRequestTimeout` / `validateStartupConfig` / `createWorkingDir` / `scheduleBackups` / `runScheduledJob`: step order, every early-return condition, every error string and wrap, the trailing-newline trim of the backup dir, the working-dir default and mode, and the scheduler options (`WithSingletonMode(LimitModeReschedule)`, plus `WithStartImmediately` only for the interval job) all match the original. `runScheduledJob` still assigns the package-level `job` variable, which `execProviderBackups` and `runProviderBackups` rely on to distinguish one-shot from scheduled runs and to print the next-run banner. The one-shot `default` arm still calls `execProviderBackups()` and returns nil.

### `internal/backup_test.go`

`TestPublicGitLabRepositoryBackup2` was confirmed to be a byte-for-byte duplicate of `TestPublicGitLabRepositoryBackup` on `main` before removal. The `TestGiteaOrgsRepositoryBackup` assertions moved into `assertGiteaOrgTwoOnlyBackedUp` and `assertGiteaAllOrgsBackedUp` preserve every directory, length and prefix check, including the double `resetBackups()` call pattern of the original.

## Questions

None.
