# Detail: internal/backup.go and internal/backup_test.go

Scope: `git diff main...review-head` for `internal/backup.go` (+246/-201) and `internal/backup_test.go` (+45/-72). File sizes after the change: `backup.go` 774 lines (729 before, so it stays well under 1k), `backup_test.go` 989 lines (was 1,016; it shrinks). No file-size threshold is crossed.

Verification: `go test ./internal/ -count=1` (offline, with the GOMODCACHE/GOCACHE/GOFLAGS=-mod=mod/GOPROXY=off/GOTOOLCHAIN=local environment) passes in about 5 seconds. Live provider tests skip because there are no credentials. Nothing exercises `displayStartupConfig`, so the claim that log output is unchanged is unverified by tests; I checked it by reading the diff line by line.

## Finding 1: the refactor spreads per-provider knowledge across parallel lists instead of modelling a provider once

Evidence: `collectProviderBackupResults` (backup.go:97) now holds a table of {token env var, run func}. Separately, `displayGitHubStartupConfig`, `displayGiteaStartupConfig`, `displayGitLabStartupConfig`, `displayBitBucketStartupConfig` and `displayAzureDevOpsStartupConfig` (backup.go:233-292) each re-state the same "is this provider's token env var set and non-empty?" guard. `checkProvider` and `enabledProviderAuth` state the env vars a third time. Each provider is therefore described in three or four unrelated places.

Measurement: `grep -c 'GetEnvOrFile(' internal/backup.go` gives 24 call sites. Most are the same `val, ok := GetEnvOrFile(x); ok && val != ""` shape, and the diff adds more of them (each display function opens with `!exists || tok == ""`).

Problem: the change satisfies the SonarQube complexity metric by extracting functions, but the reader still has to hold the same number of concepts. Adding a provider means editing the table, adding a `displayXStartupConfig`, and editing `enabledProviderAuth`. Five near-identical `displayXStartupConfig` functions differ only in which optional lines they emit.

Code-judo proposal: define a small descriptor and drive all three concerns from it.

```go
type providerSpec struct {
    label      string // "GitHub", "Azure DevOps"
    tokenEnv   string
    orgsEnv    string // "" if unsupported
    backupsEnv string // "" if unsupported
    compareEnv string
    lfsEnv     string
    run        func(string) *ProviderBackupResults
}
```

`collectProviderBackupResults` becomes a loop over `specs` with an `envDefined(spec.tokenEnv)` helper. `displayStartupConfig` becomes a loop that calls `logProvider(spec)`, which emits each optional line only when the corresponding env name is non-empty. All five display functions and all five guards disappear. Bitbucket, which has a two-mode credential check, remains a special case with its own `bitbucketAPITokenDefined` and `bitbucketOAuthDefined`. GitHub's extra `SkipUserRepos` and GitLab's minimum access level can stay as short provider-specific hooks. The `logProvider*` helpers with `(label, envVar)` signatures already point at this model, which suggests it was one step from being finished.

Remedy: follow-up refactor, not necessarily blocking, since the current code is behaviour-preserving and a strict improvement on the previous 100-line function with nested nolint directives.

## Finding 2: `createWorkingDir` re-implements the canonical `resolveWorkingDir` that sits in the same file

Evidence: `resolveWorkingDir(backupDir)` at backup.go:140 already reads `GIT_WORKING_DIR` and falls back to `filepath.Join(backupDir, workingDIRName)`. The newly extracted `createWorkingDir` (backup.go:473-480) repeats those four lines verbatim (`os.Getenv(envGitWorkingDir)`, empty check, `filepath.Join`).

Problem: the two derive the same path independently. If the default or the override rules change (for example trimming a trailing newline as `Run` does for the backup dir), one copy will be missed. The extraction was the moment to notice this; the diff instead gives the duplicate its own function name.

Remedy: replace the body's first four lines with `workingDIR := resolveWorkingDir(backupDIR)`. About 5 lines are deleted with no behaviour change.

## Finding 3: `checkProvidersDefined` keeps a counter and a two-case switch that only exist to compute a boolean

Evidence: backup.go:680-715. `count` is only compared with 0 (`if count == 0 { no providers defined }`). The `bitbucketAPITokenComplete` hoist is new, and the API-token case is now `if complete { count++ }`. The OAuth case is `if oauthDefined && !apiComplete { count++ }`. The "only count if API isn't already complete" rule exists solely to avoid double counting a value that is never used as a number.

Behaviour note: in the old code `bitbucketAPITokenComplete` was assigned inside the loop over a map, so with random map iteration the OAuth case could run first and count both Bitbucket modes (count 2 instead of 1). The hoist makes this deterministic. Because only zero versus non-zero is observable, no output changes, and the PR's "behaviour unchanged" claim is still accurate. The PR does not mention the removed nondeterminism, though.

Code-judo proposal: turn `count` into `anyDefined bool`, delete the double-count guard and the two Bitbucket switch cases by handling Bitbucket once before the loop (`anyDefined = bitbucketAPITokenDefined() || bitbucketOAuthDefined()`), and skip the two Bitbucket provider names in the loop with a single `continue`. `checkProvider` could also return `(bool, error)` for the same reason. The comment in `checkProvider` ("number of fully-configured provider entries (0 or 1)") already admits it is a boolean.

## Finding 4: helpers bake pre-existing quirks into shared code (typo, inconsistent label, env-file mismatch)

Evidence, all preserved deliberately to keep "log output unchanged":

- `logProviderOrgs` (backup.go:203-207) logs `"%s Organistations: %s"`. The typo was previously present in three separate strings and is now in one helper whose own doc comment spells it correctly. It is a one-character fix that would now be trivial. The automated reviewer flagged it and it was left unresolved.
- `logProviderBackupLFS("Gitlab", envGitLabBackupLFS)` in `displayGitLabStartupConfig` passes a different label from the `"GitLab"` used on the lines around it. The `label` parameter was introduced to remove exactly this kind of drift, so the call site should not carry it.
- `logProviderBackupLFS` tests presence with `GetEnvOrFile(envVar)` but then reads the value with `envTrue(envVar)`, which uses `os.Getenv`. A value supplied only through a `_FILE` variable would be reported as present but read as false. This is pre-existing behaviour, now centralised in one place where it can be fixed once.

Remedy: fix the typo and label in the helper and note the log-text change in the PR description, because operators grepping for the misspelled string would notice. Make `envTrue` use `GetEnvOrFile`, or drop the redundant `exists` check.

## Finding 5: test helpers: `resetBackups()` is called twice per iteration, and the helper split is thin

Evidence: in `TestGiteaOrgsRepositoryBackup` (backup_test.go, around lines 625-640) each `switch` case ends with `resetBackups()` and the loop body then calls `resetBackups()` again unconditionally. The original had the same double call. The refactor rewrote the whole block and kept it, so the cleanup is not deleted even though the code was being restructured. The automated reviewer raised it and it is unresolved.

Further: `assertGiteaOrgTwoOnlyBackedUp` and `assertGiteaAllOrgsBackedUp` each rebuild `path.Join(os.Getenv(envGitBackupDir), "gitea.lessknown.co.uk", ...)` four or five times, and each re-derives the same entries. A single `giteaOrgDir(org string) string` helper, plus a table `{org, wantDirs, notWantDirs, wantRepoPrefixes}` for the two cases, would replace both assertion functions and the switch with one loop. The switch on `org` would then disappear entirely, which is what the SonarQube complexity finding was really about.

Remedy: delete the two per-case `resetBackups()` calls (or the trailing one), add a `giteaOrgDir` helper, and consider the table-driven form.

## Finding 6: `runScheduledJob` writes to the package-global `job` through a shared helper

Evidence: `runScheduledJob` (backup.go:525-540) does `job, err = s.NewJob(...)`, assigning the package-level `var job gocron.Job` (backup.go:737). `runProviderBackups` reads `job` (`job == nil && failed > 0`, `job != nil` for NextRun). The refactor moved the assignment into a helper that takes a scheduler as a parameter, which makes the hidden global write harder to spot: the signature suggests a pure function, but it mutates process state that determines exit-code behaviour. It also declares `var err error` only to assign it with `=` to a global, which is awkward.

Remedy: have `runScheduledJob` return `(gocron.Job, error)` and let `scheduleBackups` assign the global (or better, pass the job or a "scheduled" flag into `runProviderBackups`). This is not a regression introduced by the PR, but the extraction is the natural point to fix it. Positive: merging the two near-identical scheduling blocks into `runScheduledJob` deletes about 25 lines, which is the best change in the PR.

## Positive notes

- `collectProviderBackupResults` preserves the previous provider order (Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut).
- `logProviderCompareMethod` uses `strings.EqualFold`, replacing the Bitbucket `strings.ToLower(compare) ==` spelling; the result is the same.
- Removing `TestPublicGitLabRepositoryBackup2` is correct; its body was identical to `TestPublicGitLabRepositoryBackup`.
- `Run` is decomposed cleanly into `logRequestTimeout`, `validateStartupConfig`, `createWorkingDir` and `scheduleBackups`, with error text preserved.
