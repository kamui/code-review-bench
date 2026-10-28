# Thermo nuclear review: jonhadfield/soba#195

No actionable findings.

Scope: `main...review-head` (4 files: `docker/Dockerfile`, `internal/backup.go`, `internal/backup_test.go`, `internal/notify.go`).

What I traced:

- **`collectProviderBackupResults`**: The provider order is unchanged (Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut). The credential conditions are the same as before. The table of `func(string) *ProviderBackupResults` entries compiles and `go vet` is clean.
- **`displayStartupConfig` helpers**: I compared the log lines and their order for each provider with the old code. Message text is preserved, including the existing "Organistations" typo and the "Gitlab backup LFS" casing. `strings.EqualFold` is equivalent to the old `ToLower(...) ==` check for Bitbucket. The GitHub block still has no "backups to keep" line, as before.
- **`checkProvider` split**: The just-token and user-and-password logic, error text and counts are identical.
- **`Run` split**: The order of operations is preserved: startup config, git version, request timeout, directory and provider validation, working directory, then schedule. Error wrapping is unchanged. `runScheduledJob` still assigns the package-level `job`, and the interval and cron options are the same.
- **`checkProvidersDefined`**: The Bitbucket API-token check now runs once before the map iteration. Previously it was set inside the iteration over `enabledProviderAuth`. Map order is random, so the OAuth entry could be counted twice depending on order. The result only matters when the count is zero, so this is a harmless change that makes the result deterministic.
- **`notify.go`**: `backupStatusTitle` returns strings identical to the old ones, including the leading variation-selector bytes. The three call sites now use the helper.
- **Tests**: The removed `TestPublicGitLabRepositoryBackup2` is byte-for-byte identical to `TestPublicGitLabRepositoryBackup`, apart from its name. The Gitea org test helpers keep the same assertions. `go test ./internal/ -count=1` passes offline, and the live provider tests skip.
- **Dockerfile**: Merging the `RUN` lines, sorting the apk packages and quoting the URL are behaviour-neutral. I could not build the image because Docker is unavailable. The quoted `"/var/cache/apk/*"` in `rm -f` does not glob. That line was already like that and is not part of this change.
