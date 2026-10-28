# Thermo Nuclear Review — jonhadfield/soba#195 "refactor: resolve SonarQube findings"

Range reviewed: `c77f548cbd340d2e744da7c6d92a54372b4b900b..136a4850df9aec8cf8813b27ba1533d4a17bc642` (`git diff main...review-head`), 4 files.

## Verdict

No actionable findings.

The change is a pure structural refactor. Every extracted helper was traced against the code it replaced and the observable behaviour (return values, error strings, log lines and their order, scheduler options, notification titles, Docker image contents) is unchanged. Build, vet and the `internal` package tests pass offline on the head commit, and the clone tree was unchanged afterwards.

## What was checked

**`internal/backup.go` — `runProviderBackups` / `collectProviderBackupResults`.** Provider order (Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut) and the enabling conditions are identical to the removed inline code. `bitbucketAPITokenDefined` and `bitbucketOAuthDefined` reproduce the old `apiTokenComplete` / `oauth2Complete` expressions exactly.

**`displayStartupConfig` decomposition.** For each provider the sequence of log lines was compared with the original: GitHub (orgs, skip-user-repos, compare method, LFS), Gitea (orgs, backups to keep, compare, LFS), GitLab (min access level, backups to keep, compare, LFS with the original "Gitlab" label preserved), BitBucket (backups to keep, compare, LFS), Azure DevOps (orgs, compare, LFS). `logProviderCompareMethod` emits `"<label> compare method: refs|clone"` using the `compareTypeRefs`/`compareTypeClone` constants, which are `"refs"` and `"clone"` (`internal/constants.go:108-109`), so the text matches the former literal `Print` calls. The BitBucket branch moved from `strings.ToLower(compare) == "refs"` to `strings.EqualFold(compare, "refs")`; these differ only for non-ASCII case folds (e.g. U+017F long s) and only affect a startup log line, not which comparison method is used, so this is not actionable.

**`checkProvider` split into `checkJustTokenProvider` / `checkUserAndPasswordProvider`.** Counting, the "parameter … is not defined" messages and the `count +=` accumulation are equivalent to the previous inline loops, including the `foundCount == totalCount` edge case for an empty parameter list.

**`checkProvidersDefined`.** `bitbucketAPITokenComplete` is now computed before the map iteration instead of inside the `providerNameBitBucketAPIToken` case. Under the old code, random map order could count both Bitbucket methods (count 2 instead of 1) when both were configured; the new code is deterministic. `count` is only compared with zero (`internal/backup.go:712`), so this has no user-visible effect either way.

**`Run` split into `logRequestTimeout`, `validateStartupConfig`, `createWorkingDir`, `scheduleBackups`, `runScheduledJob`.** Step order, early-return conditions, error wrapping text, and the gocron job definitions and options (`WithSingletonMode(LimitModeReschedule)`, `WithStartAt(WithStartImmediately())` for the interval case only) are unchanged. `runScheduledJob` still assigns the package-level `job` variable (`internal/backup.go:737`) and blocks in `waitForShutdown` exactly as before.

**`internal/notify.go`.** The three title constants were compared byte-for-byte with the removed literals (including the leading U+FE0F variation selector on the warning and failure titles); they are identical. `backupStatusTitle` reproduces the three-way switch and is used in the Telegram, ntfy and Slack senders in the same positions.

**`internal/backup_test.go`.** `TestPublicGitLabRepositoryBackup2` was a line-for-line duplicate of `TestPublicGitLabRepositoryBackup` and its removal loses no coverage. The `TestGiteaOrgsRepositoryBackup` helpers assert the same directories, entry counts and name prefixes as the inlined code, and keep the same `resetBackups()` calls.

**`docker/Dockerfile`.** Merging the two `RUN` layers keeps the same commands in the same order; the apk package set is the same seven packages, only reordered; quoting the `curl` URL changes nothing for the `${TAG}` values used by the release workflow. The image could not be built here (no Docker), so this is a read-only check.

## Execution

```
go build ./... && go vet ./...   -> ok
go test ./internal/ -count=1     -> ok (5.2s; credential-gated tests skipped)
git status --porcelain           -> clean
```
