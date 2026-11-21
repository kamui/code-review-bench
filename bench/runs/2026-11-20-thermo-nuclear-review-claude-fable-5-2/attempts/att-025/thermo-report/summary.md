# Thermo-nuclear review — jonhadfield/soba#195 "refactor: resolve SonarQube findings"

Range: `c77f548cbd340d2e744da7c6d92a54372b4b900b..136a4850df9aec8cf8813b27ba1533d4a17bc642` (`main...review-head`), 4 files, +312 / −305.

## Verdict

No actionable findings.

The PR claims a behaviour-preserving refactor. I traced each extracted helper against the code it replaced and found no bug, functional break, security issue, devex break or feature-gate leak introduced by the diff.

## What was checked

**`internal/backup.go`**

- `collectProviderBackupResults` runs providers in the same order as before (Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut), gated on the same environment variables with the same `ok && val != ""` test. The Bitbucket gate is still "API token complete OR OAuth complete".
- `displayStartupConfig` and its per-provider helpers: every format string was compared with the original. Labels are preserved, including the existing `Organistations` spelling and the `Gitlab backup LFS: true` casing. The same guard variable is used for each provider (Bitbucket still keys on `BITBUCKET_EMAIL`).
- `checkProvider`, `checkJustTokenProvider`, `checkUserAndPasswordProvider`: counting and error text are identical; errors accumulate into the same builder through a pointer.
- `Run`, `logRequestTimeout`, `validateStartupConfig`, `createWorkingDir`, `scheduleBackups`, `runScheduledJob`: validation order, error messages and wrapping are unchanged. `runScheduledJob` still assigns the package-level `job`, which `execProviderBackups` and `runProviderBackups` read to choose between one-shot exit and the next-run log line. The job options (`WithSingletonMode`, `WithStartAt(WithStartImmediately())` for interval only) are passed through unchanged.
- `checkProvidersDefined`: Bitbucket API-token completeness is now computed before the map loop instead of inside it. Previously, with both Bitbucket methods complete, the count was 1 or 2 depending on map iteration order; it is now always 1. The count is only compared with zero, so nothing observable changes.

**`internal/notify.go`**

- The three title constants are byte-identical to the six literals they replace, including the leading U+FE0F variation selectors (checked with `od -c` on both sides). `backupStatusTitle` has the same case order as the three removed switches.

**`internal/backup_test.go`**

- `TestPublicGitLabRepositoryBackup2` was a line-for-line duplicate of `TestPublicGitLabRepositoryBackup`; removing it loses no coverage.
- The Gitea assertion helpers make the same assertions as the inlined code (directory existence, entry counts, name prefixes).

**`docker/Dockerfile`**

- Merging the two `RUN` instructions, sorting the apk package list and quoting the download URL do not change the resulting image. `ARG TAG` is still declared before its only use. Docker is unavailable here, so the image was not built; this is from reading the file.

## Behavioural differences noted, none actionable

These are the only places where the new code is not strictly identical. None warrants a change.

- The Bitbucket startup log line now uses `strings.EqualFold(compare, "refs")` where it used `strings.ToLower(compare) == "refs"`. These differ only for non-ASCII case-fold inputs, and only in a log line.
- `bitbucketAPITokenDefined() || bitbucketOAuthDefined()` short-circuits, so the OAuth variables are not read when the API-token pair is complete. `GetEnvOrFile` has no side effect beyond logging a missing or unreadable `*_FILE` path, so at most a diagnostic line is skipped.
- Log lines carry `log.Lshortfile`, so the `backup.go:<line>` prefixes shift. The file name is unchanged.

## Verification run

- `go vet ./...` — clean.
- `go test ./internal/ -count=1` — `ok github.com/jonhadfield/soba/internal 5.225s`. Tests that need provider credentials skip in this environment, so the live backup paths (including the modified `TestGiteaOrgsRepositoryBackup`) were not exercised.
- The clone was left unmodified (`git status` clean after the run).
