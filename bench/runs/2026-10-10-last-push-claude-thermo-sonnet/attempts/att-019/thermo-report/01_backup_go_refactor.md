# Detail 01 — internal/backup.go decomposition

Scope: `internal/backup.go` (+246/−201), range `c77f548..136a485`.
Verification: read the full diff; `go vet ./internal/` clean and `go test ./internal/ -count=1` passes (offline; live-provider tests skip, so none of the provider paths below are exercised). Line numbers refer to the head file. No file crosses 1000 lines (`backup.go` is 774, `backup_test.go` is 989, `notify.go` is 238).

## Behaviour-preservation check (all confirmed by reading)

- `displayStartupConfig` split: log order per provider is unchanged (GitHub: orgs, skip-user-repos, compare, LFS; Gitea: orgs, backups, compare, LFS; GitLab: min-access, backups, compare, LFS; BitBucket: backups, compare, LFS; Azure: orgs, compare, LFS). Compare-method output is the same string via `compareTypeClone`/`compareTypeRefs` (constants.go:108-109). Provider order GitHub, Gitea, GitLab, BitBucket, Azure is preserved.
- `collectProviderBackupResults`: Bitbucket first, then Gitea, GitHub, GitLab, Azure, Sourcehut — same order as before.
- `checkProvider` helpers: same counting and error text.
- `checkProvidersDefined`: `bitbucketAPITokenComplete` is now computed before the map loop. Previously it depended on map iteration order, so the OAuth case could count when visited first. The outcome is unchanged because `count` is only compared to zero. This is a small silent fix that the PR body ("behaviour unchanged") does not mention.
- Scheduler: `runScheduledJob` still assigns the package global `job`, which `execProviderBackups` reads at backup.go:35.

## Finding F1 — The decomposition rearranges the per-provider repetition instead of deleting it

Evidence: backup.go:233-298 has five `display<Provider>StartupConfig` functions. Each opens with the same "get token env, `!exists || v == ""` → return" guard. Each then calls the same three or four `logProvider*` helpers with a `(label, envVar)` pair. Three of the helpers (`logProviderOrgs`, `logProviderBackupsToKeep`, `logProviderBackupLFS`) share the shape "read env, maybe log". `collectProviderBackupResults` (97-122) repeats the same provider-to-env mapping again as a `tokenProviders` table. `checkProvider` uses `justTokenProviders`/`userAndPasswordProviders`/`enabledProviderAuth` from constants.go, which is a third parallel description of "which providers exist and what env activates them".

Problem: the cognitive-complexity warning went away, but the reader must now hold five near-identical functions plus a table. A new provider means editing four places (constants maps, the collect table, a new `display*` function, `displayStartupConfig`).

Code-judo proposal: introduce one descriptor type, for example

```go
type providerSpec struct {
    label, tokenEnv, orgsEnv, backupsEnv, compareEnv, lfsEnv string
    run func(string) *ProviderBackupResults
}
```

Hold one `[]providerSpec`. `displayStartupConfig` becomes a single loop: if the token is set, call `logProviderOrgs` only when `orgsEnv != ""`, and so on. `collectProviderBackupResults` iterates the same slice. The five `display*` functions disappear. Outliers (GitHub skip-user-repos, GitLab min access level, BitBucket's two auth modes) stay as small optional hooks on the spec. This deletes roughly 60 lines and the "four places to edit" problem rather than relocating it.

Verification: structural reading; not compiled.

## Finding F2 — Repeated "env is set and non-empty" predicate has no helper

Evidence: the pattern `v, ok := GetEnvOrFile(X); ok && v != ""` (or its negation) appears at backup.go:57, 108, 126-135, 234, 247, 260, 277, 288, and in `checkProvider*`. The PR added about eight new copies of the negated form inside the new guard clauses.

Remedy: add `envDefined(name string) bool` (and optionally `envDefinedTrimmed`, since `checkProvider` uses `strings.Trim(val, " ") == ""`). `bitbucketAPITokenDefined` and `bitbucketOAuthDefined` then shrink to one line each, and every guard becomes `if !envDefined(envGiteaToken) { return }`. This also removes the unused-variable noise (`ghToken, exists := ...; !exists || ghToken == ""`).

Verification: grep over the head file.

## Finding F3 — `createWorkingDir` re-implements the existing `resolveWorkingDir`

Evidence: `resolveWorkingDir(backupDir)` at backup.go:139 already returns `GIT_WORKING_DIR` or `<backupDir>/<workingDIRName>`. The new `createWorkingDir` (473-486) repeats the same `os.Getenv(envGitWorkingDir)` / `filepath.Join(backupDIR, workingDIRName)` logic. The PR extracted this function from `Run` and did not look next door.

Remedy: `workingDIR := resolveWorkingDir(backupDIR)`, then log and `MkdirAll`. The two code paths can no longer drift (for example if the default directory name changes, or `GetEnvOrFile` support is added to one but not the other). This is a canonical-helper-duplication case from the skill's blocker list.

Verification: read both functions; they are textually the same decision.

## Finding F4 — `scheduleBackups`/`runScheduledJob` keep a hidden global and build a scheduler for the one-shot path

Evidence: `runScheduledJob` (525-541) writes the package global `job` (backup.go:737) as a side effect, and `execProviderBackups` (35) uses `job == nil` as a "one-shot mode" flag. That is why `var err error` plus `job, err =` is needed instead of `j, err :=`. The helper's name and signature (`s, definition, options...`) hide that it mutates global state which changes process-exit behaviour. `scheduleBackups` (491-521) also creates a `gocron` scheduler before the `switch`, so the `default:` one-shot branch builds a scheduler it never starts or shuts down (pre-existing, but now more visible). Control flow mixes `return runScheduledJob(...)` in two cases with a fall-through `return nil` for the default case, and `runScheduledJob` returns `nil` only after blocking on a signal.

Code-judo proposal: pass the mode explicitly. `execProviderBackups(scheduled bool)` (or a closure) removes the `job` global and the `var err error` workaround. Create the scheduler inside `runScheduledJob(definition, options...)` so `scheduleBackups` becomes a pure selector that returns `(definition, options, ok)`. The one-shot path then never touches gocron.

Verification: read; behaviour-neutral by inspection. Not run, because the scheduler path blocks on signals.

## Finding F5 — `validateStartupConfig` is a mixed bag whose name hides a return value and a normalisation

Evidence: backup.go:443-470 reads `os.LookupEnv(envGitBackupDir)` (not `GetEnvOrFile`, unlike `runProviderBackups` at line ~50), checks the GitHub orgs/token pair, mutates the value with `TrimSuffix(backupDIR, "\n")`, stats the directory, and then calls `checkProvidersDefined`. It returns the trimmed directory. A function called "validate" that also normalises and returns the canonical backup dir is surprising. Two different env-reading conventions for the same variable (`LookupEnv` here, `GetEnvOrFile` in `runProviderBackups`) now sit next to each other.

Remedy: split into `resolveBackupDir() (string, error)` (lookup, trim, stat) and `validateProviderConfig() error` (orgs/token pair, `checkProvidersDefined`). Use the same env accessor in both places, or add a short comment explaining why file-based secrets are not supported for the directory.

Verification: read; the `LookupEnv`/`GetEnvOrFile` inconsistency is pre-existing but was carried into the new helper unexamined.

## Finding F6 — Label inconsistency now sits in a shared helper's call site

Evidence: backup.go:273 calls `logProviderBackupLFS("Gitlab", ...)` while the compare and backups lines for the same provider use `"GitLab"`. The "Organistations" typo is also preserved in `logProviderOrgs` (backup.go:205). Preserving output was the stated goal, but a single `label` parameter per provider (the descriptor in F1) makes the discrepancy structurally impossible.

Verification: grep; user-visible log text, so changing it is a decision for the author.

## Finding F7 — No test covers the refactored code that the PR says is behaviour-neutral

Evidence: `grep` over `internal/*_test.go` shows only `checkProvidersDefined` has a direct test. None for `displayStartupConfig`, `collectProviderBackupResults`, `checkProvider`, `checkJustTokenProvider`, `checkUserAndPasswordProvider`, `bitbucket*Defined`, `scheduleBackups`. The live tests skip without credentials, and in this review environment they all skipped. "Log output is unchanged" rests on the author's reading.

Remedy: capture `logger` output into a buffer and table-test the startup config for a given environment, and table-test `checkProvider` for the partial-credential cases. The extraction into helpers makes this easy, and it would catch the label drift in F6.

Verification: grep; `go test ./internal/ -count=1` passes but does not exercise these paths.
