# Detail 01: internal/backup.go and internal/notify.go

## Measurements and commands

- `git diff main...review-head -- internal/backup.go internal/notify.go docker/Dockerfile` for the full diff.
- `wc -l`: `internal/backup.go` 774 lines (729 on main), `internal/notify.go` 238, `internal/backup_test.go` 989 (was 1016 on main; the file stays below 1000 lines).
- `go build ./...`, `go vet ./internal/`, `go test ./internal/ -count=1` (offline, GOFLAGS=-mod=mod GOPROXY=off): all pass. Provider tests skip without credentials, so the refactored startup and provider-collection paths are exercised only by the tests that do not need credentials.
- `grep -n "job\b" internal/*.go`: `job` is read at `backup.go:35` and `backup.go:87`, written at `backup.go:528`, declared at `backup.go:737`.
- `cat -A` on `notify.go:26-28`: the second and third title constants begin with bytes `EF B8 8F` (U+FE0F), the first does not.

## Behaviour-preservation audit (all verified by reading)

- Provider order in `collectProviderBackupResults`: Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut. Same as before.
- Startup log lines: same text and same order per provider. Gitea's `logger.Print("Gitea compare method: refs")` became `logger.Printf("%s compare method: %s", ...)`, identical output. GitLab's LFS label stays "Gitlab" (lowercase l) via a separate call; this is preserved, not fixed.
- `checkProvider` split: `checkJustTokenProvider` and `checkUserAndPasswordProvider` match the original loops. A provider in both lists would double-count exactly as before.
- `checkProvidersDefined`: originally `bitbucketAPITokenComplete` was assigned inside the map iteration, so the OAuth case's "only count if API token not complete" depended on Go's random map order; now it is computed up front. This is a benign determinism improvement, and because `count` is only compared with zero it cannot change any outcome. The PR says behaviour is unchanged; strictly it removed a latent order dependency and did not say so.
- `Run` split: order of operations is identical (git path, git version, timeout, validation, mkdir, scheduling). The end-of-`Run` `return nil` is reached the same way after a scheduler shutdown.

## Finding 1: five parallel display functions (`backup.go:233-293`)

Structure today:

```
displayGitHubStartupConfig:      gate(GH token); orgs; skip-user-repos; compare; LFS
displayGiteaStartupConfig:       gate(Gitea token); orgs; backups; compare; LFS
displayGitLabStartupConfig:      gate(GL token); min-access-level; backups; compare; LFS
displayBitBucketStartupConfig:   gate(BB email); backups; compare; LFS
displayAzureDevOpsStartupConfig: gate(AZ user); orgs; compare; LFS
```

Worked code-judo proposal:

```go
type providerStartupLog struct {
    label, gate                       string
    orgs, backups, compare, backupLFS string // empty = not applicable
}

var startupLogs = []providerStartupLog{
    {"GitHub", envGitHubToken, envGitHubOrgs, "", envGitHubCompare, envGitHubBackupLFS},
    {"Gitea", envGiteaToken, envGiteaOrgs, envGiteaBackups, envGiteaCompare, envGiteaBackupLFS},
    // ...
}
```

One loop over `startupLogs` calls the four existing `logProvider*` helpers when the field is non-empty. The three genuine special cases (GitHub skip-user-repos, GitLab min access level, and Bitbucket's gate) are handled by an optional `extra func()` field. This deletes the five gate blocks and five functions, leaves the existing `logProvider*` helpers in place, and can be shared with `collectProviderBackupResults`'s `tokenProviders` slice (same label and gate variable).

Verification status: proposal not compiled; equivalence argued from the current code.

## Finding 2: predicate duplication (`backup.go`)

Occurrences of the "defined and non-empty" shape in the changed code: 5 in `collectProviderBackupResults` and the Bitbucket helpers combined with 8 in the display functions/`logProvider*` and 2 to 3 more (`Trim` variants) in `checkJustTokenProvider`/`checkUserAndPasswordProvider`. Proposal: `func envDefined(name string) bool { v, ok := GetEnvOrFile(name); return ok && v != "" }`.

`displayBitBucketStartupConfig` gates on `envBitBucketEmail` only, while `collectProviderBackupResults` runs Bitbucket when either API-token or OAuth credentials are complete. OAuth-only users therefore get no Bitbucket startup logging. Pre-existing; question raised in the summary as 2b.

## Finding 3: `createWorkingDir` vs `resolveWorkingDir` (`backup.go:140-146, 473-485`)

Both compute `GIT_WORKING_DIR` or `filepath.Join(backupDir, workingDIRName)`. `cleanupWorkingDir` (which deletes, with a containment check) uses `resolveWorkingDir`; `createWorkingDir` (which creates) does not. Fix: `workingDIR := resolveWorkingDir(backupDIR)`. Note the subtle difference that `backupDIR` in `Run` is trimmed of a trailing newline but `runProviderBackups` reads it untrimmed via `GetEnvOrFile`; both go through the same join, so the paths only agree if `GetEnvOrFile` trims. That is worth a look but is out of scope for this PR.

## Finding 4: `job` global and start-immediately race (`backup.go:35, 87, 525-540, 737`)

Sequence in the interval path: `s.NewJob(..., WithStartAt(WithStartImmediately()))` registers the job, `job = <returned>`, then `s.Start()`. The job function `execProviderBackups` runs on a gocron goroutine after `Start`. `NewJob` returns before `Start`, so the assignment normally wins, and I did not find a reproducer; the hazard is that the read and write are unsynchronised and the cost of losing is a process-wide `os.Exit(1)`. Rated as a design smell with a latent race, not a demonstrated bug. Proposal in the summary: `execProviderBackups(oneShot bool)`, delete the global, pass the next-run banner callback or return value.

## Finding 5: `Run` decomposition (`backup.go:396-520`)

`validateStartupConfig` returns `(string, error)` and also normalises the backup dir; `logRequestTimeout` validates; `scheduleBackups` re-reads `GIT_BACKUP_INTERVAL` and `GIT_BACKUP_CRON`. Proposal: `type startupConfig struct { backupDir, workingDir string; reqTimeout time.Duration; interval int; cron string }` produced by one `loadStartupConfig() (startupConfig, error)`, after which `Run` is log, mkdir, schedule with no environment access.

## Finding 6: duplicated three-way status (`backup.go:74-81`, `notify.go:31-39`)

`runProviderBackups` switch: `succeeded == 0 && failed >= 0` -> all failed; `succeeded > 0 && failed > 0` -> errors; default -> complete. `backupStatusTitle`: `succeeded>0 && failed==0` -> succeeded; `failed>0 && succeeded>0` -> errors; default -> failed. Same partition, expressed twice with different guard ordering. `failed >= 0` is always true for a count, which suggests the condition was written for the compiler and not for the reader. Proposal:

```go
type backupStatus int
const (statusSucceeded backupStatus = iota; statusPartial; statusFailed)
func statusOf(succeeded, failed int) backupStatus { ... }
```

with the log message and the title both selected by `switch statusOf(...)`.

## Finding 7: typo and stray U+FE0F

`internal/backup.go:205`: `"%s Organistations: %s"`. `internal/notify.go:27-28`: titles begin with `EF B8 8F`. Both are byte-for-byte carried from the original inline strings (verified with `git show main:internal/notify.go`). No tests assert on either string.

## Approval-bar check

- File crossing 1000 lines: no.
- New ad-hoc branching in shared paths: no.
- Preserves incidental complexity where a judo move exists: yes (findings 1, 2, 4, 5, 6).
- Duplicates an existing helper: yes (finding 3).
