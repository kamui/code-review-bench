# Thermo nuclear review: jonhadfield/soba#195

No actionable findings.

Scope: `main...review-head`, one commit, 4 files (`docker/Dockerfile`, `internal/backup.go`, `internal/backup_test.go`, `internal/notify.go`).

What I traced:

- **`collectProviderBackupResults`**: the provider table keeps the original order (Gitea, GitHub, GitLab, Azure DevOps, Sourcehut) after Bitbucket. Each provider's credential check is the same as before, and the Bitbucket predicates are the original conditions moved into helpers.
- **Startup config logging**: every provider's log lines, their order and their wording match the original. This includes the existing "Organistations" typo and the "Gitlab backup LFS" casing. The `compareTypeClone` and `compareTypeRefs` constants are "clone" and "refs", which equal the old literals. Bitbucket's `ToLower == refs` check and `EqualFold` give the same result.
- **`checkProvider`**: the helpers produce the same counts and error text. The error builder is now passed by pointer, and the final error return is unchanged.
- **`Run` split**: the order of steps is preserved. The global `job` is still set, and `runProviderBackups` still reads it. Interval and cron scheduling still block in `waitForShutdown`, and the default case still runs a single backup.
- **`checkProvidersDefined`**: Bitbucket API-token completeness is now computed before the loop instead of inside it. The old result depended on map iteration order, because the OAuth case could run before the API-token case. The count is only used to test for zero and nothing else reads it, so the outcome is unchanged and the order dependence is gone.
- **`notify.go`**: `backupStatusTitle` has the same switch conditions and title strings, including the leading variation-selector characters. The constants contain the same strings.
- **Tests**: the removed `TestPublicGitLabRepositoryBackup2` is a line-for-line duplicate of `TestPublicGitLabRepositoryBackup`. The Gitea org assertions were moved into helpers without changing any of them.
- **Dockerfile**: merging the `RUN` steps, sorting the apk packages and quoting the `${TAG}` URL do not change what gets installed or downloaded. The old `rm -f "/var/cache/apk/*"` was already an unexpanded quoted glob, so it was already a no-op. I could not build the image because Docker is unavailable.

I did not run the test suite. The live provider tests skip without credentials.
