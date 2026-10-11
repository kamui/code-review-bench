# 01 internal/backup.go

Measurements: file went from 729 to 774 lines (under the 1k threshold). `git diff main...review-head -- internal/backup.go` shows +246/-201. Offline `go test ./internal/ -count=1` passes.

## Per-provider startup logging (findings 1)
Evidence: backup.go:203-295. Each of `displayGitHubStartupConfig`, `displayGiteaStartupConfig`, `displayGitLabStartupConfig`, `displayBitBucketStartupConfig` and `displayAzureDevOpsStartupConfig` is a guard on a token or user env var followed by 2-4 calls to the `logProvider*` helpers. `collectProviderBackupResults` (backup.go:97-123) separately carries a table `{envVar, run}` of the same providers.

Worked code-judo proposal: one descriptor
```go
type providerCfg struct{ label, tokenEnv, orgsEnv, keepEnv, compareEnv, lfsEnv string; run func(string) *ProviderBackupResults }
var providers = []providerCfg{...}
```
`displayStartupConfig` ranges over it and logs only the fields that are non-empty. `collectProviderBackupResults` ranges over the same slice. GitHub's skip-user-repos line and GitLab's min-access level stay as small optional hooks. Quirks (the "Organistations" typo, "Gitlab" label, Azure having no backups line, Bitbucket OAuth not logged) become one-line table edits instead of five hidden functions. Verification status: static reading of the diff, not executed. Log text equivalence would need a log-capturing test.

## Scheduler (finding 2)
Evidence: backup.go:491-541 and the `var job gocron.Job` global at backup.go:737, read at lines 35 and 87-88. `runScheduledJob` assigns `job`, starts the scheduler and blocks in `waitForShutdown`.

Worked proposal:
```go
var def gocron.JobDefinition; var opts []gocron.JobOption
switch { case interval != 0: def, opts = ...; case cron != "": def, opts = ...; default: execProviderBackups(); return nil }
job, err = s.NewJob(def, gocron.NewTask(execProviderBackups), opts...)
```
followed by `s.Start()` and `waitForShutdown(s)`. This removes the variadic wrapper and the redundant `var err error`. The scheduler would be created only on the scheduled arms. Verification: static reading.

## Run decomposition (finding 3)
Evidence: backup.go:426-488. `validateStartupConfig` returns the trimmed `backupDIR`, so validation and normalisation are fused. `createWorkingDir` has no doc comment and duplicates the `GIT_WORKING_DIR`-or-default logic of `resolveWorkingDir`. Verification: static reading of both functions.

## Credential helpers (findings 4 and 5)
Evidence: backup.go:348-395, 680-715. The `*strings.Builder` out-parameters are used alongside return values. `checkProvider` guards with two independent `slices.Contains` checks. In `checkProvidersDefined` the old lazily-set `bitbucketAPITokenComplete` depended on map iteration order. The new code precomputes it, a behaviour change not mentioned in the PR body. Proposal: have the helpers return `(count, missing []string)`, and add a test with both Bitbucket auth styles configured expecting a count of 1. Verification: reasoning from the old and new code.
