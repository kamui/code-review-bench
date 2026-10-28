# Thermo Nuclear Review — jonhadfield/soba#195 "refactor: resolve SonarQube findings"

Range: `c77f548c..136a4850` (`main...review-head`), 4 files: `docker/Dockerfile`, `internal/backup.go`, `internal/backup_test.go`, `internal/notify.go`.

No actionable findings.

## What was traced

**`internal/backup.go` — `runProviderBackups` → `collectProviderBackupResults`.** Provider order (Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut) and the gating conditions (`ok && val != ""` via `GetEnvOrFile`) are unchanged. The Bitbucket check is now `bitbucketAPITokenDefined() || bitbucketOAuthDefined()`. Because of short-circuiting, the three OAuth variables are no longer read when the API-token pair is complete. The only side effect of `GetEnvOrFile` (`internal/envfile.go:14`) is a log line when a `*_FILE` path is missing, so the only change is that some redundant log lines may no longer appear. Results are the same.

**`checkProvidersDefined`.** `bitbucketAPITokenDefined()` is now computed before the loop over the `enabledProviderAuth` map. Before, `bitbucketAPITokenComplete` was set inside the loop, so a random map order that visited `providerNameBitBucketOAuth` first could count Bitbucket twice. Now the result doesn't depend on order. `count` is only compared against `0` ("no providers defined"), so neither the old nor the new count changes the outcome. No regression.

**`checkProvider` → `checkJustTokenProvider` / `checkUserAndPasswordProvider`.** Both helpers get `*strings.Builder` (`&outputErrs`), so error accumulation works as before. The `continue` on `!exists` keeps the old nested-if semantics. The user/password helper returns 1 only when `foundCount == totalCount`, which matches the old `count++`.

**`Run` → `logRequestTimeout` / `validateStartupConfig` / `createWorkingDir` / `scheduleBackups` / `runScheduledJob`.** Validation order and error messages are preserved. The backup dir returned by `validateStartupConfig` has `\n` trimmed, as the old code did before it built the working-dir path. `runScheduledJob` still assigns the package-level `job` (`internal/backup.go:737`), which `runProviderBackups`/`execProviderBackups` read at lines 35 and 87, so next-run logging and the `job == nil && failed > 0` exit path still work. Interval jobs still get `WithSingletonMode(LimitModeReschedule)` and `WithStartImmediately()`. Cron jobs still get singleton mode only. The scheduler still starts, and `waitForShutdown` still blocks. The no-schedule path still runs `execProviderBackups()` once.

**`displayStartupConfig` helpers.** I compared the log text for each provider: "Organistations" (original spelling), "backups to keep", "compare method: refs|clone" (`compareTypeRefs`/`compareTypeClone` = `"refs"`/`"clone"`, `internal/constants.go:108-109`), "Gitlab backup LFS" (original casing kept), and the GitLab min-access-level line. Each provider still logs the same set of lines as before: Bitbucket logs no orgs and Azure logs no backups-to-keep. Bitbucket's compare check changed from `ToLower(x) == "refs"` to `EqualFold`. This only affects a startup log line, and for ASCII input the two are equivalent.

**`internal/notify.go`.** I compared the bytes of the three title literals before and after (including the leading U+FE0F variation selectors) and they are identical. `backupStatusTitle` keeps the same switch order, and Telegram, ntfy and Slack all use it.

**`internal/backup_test.go`.** The removed `TestPublicGitLabRepositoryBackup2` matched `TestPublicGitLabRepositoryBackup` token for token, so no coverage is lost. The refactored Gitea org assertions check the same directories, entry counts and name prefixes.

**`docker/Dockerfile`.** User creation and `apk add` are now one `RUN`, and the package set is unchanged (it's only reordered). Quoting `${TAG}` inside the URL doesn't change how it expands. The quoted `rm -f "/var/cache/apk/*"` glob was there before this change, and `--no-cache` makes it moot anyway. I couldn't build the image because Docker isn't available.

**Security / feature gates / devex.** No env var names, secret sources, ports or gates changed.

## Verification run

`go vet ./...` was clean, and `go test ./internal/ -count=1` passed (offline; tests that need credentials skipped). The working tree stayed clean afterwards.
