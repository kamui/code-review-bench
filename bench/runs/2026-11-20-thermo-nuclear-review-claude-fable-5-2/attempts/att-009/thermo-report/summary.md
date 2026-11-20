# Thermo-nuclear review — jonhadfield/soba#195 "refactor: resolve SonarQube findings"

Range: `c77f548cbd340d2e744da7c6d92a54372b4b900b..136a4850df9aec8cf8813b27ba1533d4a17bc642` (4 files, +312 / −305).

No actionable findings.

The PR claims a behaviour-preserving refactor. I traced each changed path against the base and found no bug, breaking change, security issue, devex regression or feature-gate leak introduced by the diff.

## What was checked

**`internal/backup.go`**

- `collectProviderBackupResults` (lines 95–123): providers still run in the base order (Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut), with the same env vars and the same `ok && val != ""` gates. The Bitbucket gate is the same API-token-or-OAuth condition, now via `bitbucketAPITokenDefined` / `bitbucketOAuthDefined`.
- `displayStartupConfig` and its helpers (lines 190–294): every log line matches the base text, including the "Organistations" spelling and the `Gitlab backup LFS` label. `compareTypeRefs` / `compareTypeClone` are `"refs"` / `"clone"`, so the compare lines are unchanged.
  - The only semantic difference is that the Bitbucket compare log now uses `strings.EqualFold` where the base used `strings.ToLower(...) ==`. These differ only for the non-ASCII long s (`ſ`) in a log line, so it is not actionable.
  - The logger uses `log.Lshortfile`, so the `backup.go:NNN` prefix on these lines changes, as it would with any edit to the file.
- `checkProvider`, `checkJustTokenProvider`, `checkUserAndPasswordProvider` (lines 326–395): counts and error text are the same as the base; no provider is in both `justTokenProviders` and `userAndPasswordProviders`.
- `checkProvidersDefined` (lines 680–717): `bitbucketAPITokenComplete` is now computed before the map loop instead of inside the `BitBucketAPIToken` case.
  - On the base, random map order could count Bitbucket twice when both auth methods were complete.
  - `count` is only compared with zero, and both provider keys exist in `enabledProviderAuth`, so the result is the same for every input.
- `Run` and its helpers `logRequestTimeout`, `validateStartupConfig`, `createWorkingDir`, `scheduleBackups`, `runScheduledJob` (lines 397–541): the order of checks, error wrapping and messages, and the `\n` trim of the backup directory are preserved.
  - The package-level `job` is still assigned before `s.Start()`, so the one-shot `os.Exit(1)` and next-run logging in `execProviderBackups` / `runProviderBackups` behave as before.
  - Job definition, task and options reach `s.NewJob` in the same order.
  - The default one-shot path still returns `nil`.

**`internal/notify.go`**

- The three title constants are byte-identical to the six literals they replace, including the leading U+FE0F on the warning and failure titles (checked with `od -c` on both revisions).
- `backupStatusTitle` has the same case order and default as the three removed switches.

**`internal/backup_test.go`**

- The removed `TestPublicGitLabRepositoryBackup2` has the same body as `TestPublicGitLabRepositoryBackup`, and nothing else in the repo refers to it by name.
- The new Gitea helpers make the same assertions as the inline code they replace.

**`docker/Dockerfile`**

- The merged `RUN`, the sorted apk package list (same seven packages) and the quoted `${TAG}` URL are equivalent in shell form. The image could not be built here because Docker is unavailable; this was checked by reading only.

## Execution evidence

- `go vet ./...` on the head: clean.
- `go test ./internal/ -count=1` on the head: `ok` (live provider tests skip without credentials).
- Differential check in scratch copies of base and head under the work directory: 20,000 seeded random environment combinations across 31 provider variables, capturing `displayStartupConfig` output, the `checkProvidersDefined` outcome and per-provider `checkProvider` results. The two outputs (345,607 lines each) were byte-identical.
- Not exercised: the scheduler paths (interval and cron), which block until a signal, and the live backup runs. Those were verified by reading.
