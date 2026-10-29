# Thermo Nuclear Review — jonhadfield/soba#195 "refactor: resolve SonarQube findings"

Range reviewed: `c77f548cbd340d2e744da7c6d92a54372b4b900b..136a4850df9aec8cf8813b27ba1533d4a17bc642` (`git diff main...review-head`), 4 files, +312 / −305.

## Verdict

No actionable findings.

## What was audited

The PR is a pure structural refactor driven by SonarQube cognitive-complexity, duplicated-literal, duplicated-test and Dockerfile lint rules. I traced every extracted helper against the code it replaced to confirm behaviour, ordering, log text and error text are unchanged, and ran the module offline: `go build ./...`, `go vet ./...` and `go test ./internal/ -count=1` all pass (the credential-gated live backup tests skip, as expected).

### internal/backup.go

- `runProviderBackups` → `collectProviderBackupResults` (lines 95–122): provider order (Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut) and the `ok && val != ""` gating are identical to the removed inline code. The Bitbucket predicate is split into `bitbucketAPITokenDefined` / `bitbucketOAuthDefined` (lines 124–138) with the same five env lookups and the same non-empty checks.
- `displayStartupConfig` → per-provider `display*StartupConfig` functions (lines 195–293). For each provider I compared the sequence and text of every log line against the original: GitHub (orgs, skip-user-repos, compare, LFS), Gitea (orgs, backups-to-keep, compare, LFS), GitLab (min access level, backups-to-keep, compare, LFS with the original lowercase "Gitlab" label preserved), BitBucket (backups-to-keep, compare, LFS), Azure DevOps (orgs, compare, LFS). `logProviderCompareMethod` formats `"%s compare method: %s"` with `compareTypeRefs`/`compareTypeClone` (`"refs"`/`"clone"` in `constants.go:108-109`), which reproduces the previous literal strings exactly. The only comparator change is BitBucket moving from `strings.ToLower(x) == "refs"` to `strings.EqualFold(x, "refs")`, which the other four providers already used; it affects only a startup log line and not which compare path runs.
- `checkProvider` → `checkJustTokenProvider` / `checkUserAndPasswordProvider` (lines 329–395): counting, the "defined but blank" error emission for token providers, the partial-definition error emission for user/password providers, and the all-or-nothing `+1` are unchanged; `checkProvider` still returns the aggregated error only when the builder is non-empty.
- `checkProvidersDefined` (lines 680–717): `bitbucketAPITokenComplete` is now computed before the map loop instead of inside the `providerNameBitBucketAPIToken` case. Because `enabledProviderAuth` is a map with random iteration order, the old code could count both Bitbucket methods when the OAuth case ran first; the new code is deterministic. `count` is only tested against zero, so this is behaviour-preserving and strictly less surprising.
- `Run` → `logRequestTimeout`, `validateStartupConfig`, `createWorkingDir`, `scheduleBackups`, `runScheduledJob` (lines 397–541): the step order (git lookup, startup display, timeout, backup dir presence, GitHub orgs/token dependency, trailing-newline trim, stat, provider check, working-dir creation, scheduler) and every error message are identical. `validateStartupConfig` returns the trimmed `backupDIR`, so `createWorkingDir` derives the default working directory from the same value as before. `runScheduledJob` assigns the package-level `job` variable (`backup.go:737`) via `job, err = s.NewJob(...)` with a local `var err error`, so the `job == nil` branch in `execProviderBackups` (line 35) and `job.NextRun()` in `runProviderBackups` (line 88) still see the scheduled job. The interval case still passes `WithSingletonMode` and `WithStartImmediately`; the cron case still passes only `WithSingletonMode`; the default case still runs a single synchronous backup.

### internal/notify.go

`backupStatusTitle` (lines 31–40) and the three title constants reproduce the former `switch` in `sendTelegramMessage`, `sendNtfy` and `sendSlackMessage` byte-for-byte, including the pre-existing leading U+FE0F in the warning and failure titles, so outgoing notification text is unchanged across all three channels.

### internal/backup_test.go

`TestPublicGitLabRepositoryBackup2` was a verbatim duplicate of `TestPublicGitLabRepositoryBackup` (same skip guard, same env isolation, same `Run()` assertion); removing it loses no coverage. The `TestGiteaOrgsRepositoryBackup` assertion helpers check the same directories, entry counts and name prefixes as the inlined code, and the `resetBackups()` calls are preserved in the same places.

### docker/Dockerfile

Merging the user-creation `RUN` into the `apk add` `RUN`, alphabetising the package list (same seven packages), and quoting the download URL containing `${TAG}` do not change the resulting image or the build inputs. No ports, env vars, secrets or run commands changed. Docker is unavailable in this environment so the image was not built; the change is syntactically trivial.

## Devex, feature-gate and security notes

No environment variable names, secret sources, ports, scripts or run commands changed. No feature gates exist in the touched code. No new input handling or external calls were introduced.
