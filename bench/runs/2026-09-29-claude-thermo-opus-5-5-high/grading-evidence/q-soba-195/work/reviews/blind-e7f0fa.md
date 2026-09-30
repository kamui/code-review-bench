# Review blind-e7f0fa

### Item 1
Location: internal/backup.go:105-294
Claim: **1. Provider knowledge is now split across four tables and five cloned display functions. A single provider descriptor would delete most of them.** (P1, detail: `01_provider_config.md` §P1-A)
Per-provider facts (label, gating credential, orgs/backups/compare/LFS env vars, backup entry point) were already duplicated between `internal/constants.go:120-159` and hand-written blocks in `backup.go`. The PR adds another copy, the anonymous `tokenProviders` slice in `collectProviderBackupResults` (`internal/backup.go:105-114`). It also splits `displayStartupConfig` into five functions (`internal/backup.go:233-294`) that each repeat the same guard-then-four-log-helpers pattern, with the label and env constants passed in by hand. Drift is already visible: GitLab's LFS line is labelled `"Gitlab"` (line 273), while the three lines above it say `"GitLab"`. Sourcehut is backed up but never shown in startup config. The fix is one `providerSpec` table (label, `enabled func() bool`, `run`, optional env vars, optional extra-log hook) that both `collectProviderBackupResults` and `displayStartupConfig` loop over. That deletes the five display functions and the ad-hoc table, and it makes adding a provider a single edit. The worked sketch is in the detail file.
Consequence: —
Fix: —

### Item 2
Location: internal/backup.go:276-284
Claim: **2. Bitbucket startup display uses a different "is Bitbucket configured" rule than the predicates this PR just extracted.** (P1, detail: `01_provider_config.md` §P1-B)
The PR adds `bitbucketAPITokenDefined()` / `bitbucketOAuthDefined()` (`internal/backup.go:125-138`) and uses them for backup selection and validation. `displayBitBucketStartupConfig` (`internal/backup.go:276-284`) still gates only on `BITBUCKET_EMAIL`. An OAuth-only Bitbucket user gets backed up with no startup-config lines. An email-only user gets config lines and no backup. This behaviour is inherited from base, but the PR created the canonical predicate and then skipped it at the third site. Add `bitbucketDefined()` and use it everywhere. With finding 1 it becomes the Bitbucket entry's `enabled` field.
Consequence: —
Fix: —

### Item 3
Location: internal/backup.go:473-486
Claim: **3. `createWorkingDir` re-implements `resolveWorkingDir`.** (P1, detail: `02_run_orchestration.md` §P1-C)
The newly named `createWorkingDir` (`internal/backup.go:473-486`) repeats the `GIT_WORKING_DIR`-or-`filepath.Join(backupDir, workingDIRName)` fallback that `resolveWorkingDir` (`internal/backup.go:140-146`) already provides. `runProviderBackups` uses `resolveWorkingDir` to pick the directory that `cleanupWorkingDir` later passes to `os.RemoveAll`. Two definitions of that path, one for create and one for delete, are a real maintenance hazard. Replace the block with `resolveWorkingDir(backupDIR)`.
Consequence: —
Fix: —

### Item 4
Location: internal/backup.go:680-717
Claim: **4. `checkProvidersDefined` keeps a loop-switch over map keys, and the extracted `checkProvider` helpers keep dead and duplicated paths.** (P2, detail: `01_provider_config.md` §P2-A)
`checkProvidersDefined` (`internal/backup.go:680-717`) ranges over `enabledProviderAuth` only to special-case two keys through a `switch`. Its "only count once" Bitbucket branch protects an invariant nobody reads, because `count` is only compared with zero. `userAndPasswordProviders` lists both Bitbucket providers, but `checkProvider` is never called for them, so `checkUserAndPasswordProvider` (lines 369-395) only ever sees Azure DevOps, despite its doc comment. That helper was extracted verbatim and still scans every parameter twice. Both new helpers return a count *and* write into a `*strings.Builder` out-parameter. The simpler shape is to handle Bitbucket once outside the loop, loop over the remaining providers, and use a single-pass `missingParams`/`blankParams` helper that returns the offending names. That removes the out-parameter, the double scan and the switch. Add a table test for `checkProvidersDefined` first, because none exists.
Consequence: —
Fix: —

### Item 5
Location: internal/backup.go:491-543
Claim: **5. `runScheduledJob` hides the only write of the global `job`, and `scheduleBackups` builds a scheduler it may never use.** (P2, detail: `02_run_orchestration.md` §P2-B)
`job == nil` is how `execProviderBackups` decides whether to `os.Exit(1)`. At base that global was assigned visibly inside `Run`. Now it is assigned as a side effect of `runScheduledJob` (`internal/backup.go:525-543`), whose name and doc comment do not mention it. `scheduleBackups` (lines 491-521) also calls `gocron.NewScheduler()` before deciding whether it is in one-shot mode, and both scheduled arms repeat `WithSingletonMode(LimitModeReschedule)`. Choose the job definition first, return early for one-shot, then create the scheduler, register the job with the shared option, and assign `job` in `scheduleBackups` itself. That deletes `runScheduledJob` and the wasted scheduler. Passing "scheduled" into the task through a closure would remove the global entirely.
Consequence: —
Fix: —

### Item 6
Location: internal/notify.go:31-40
Claim: **6. `backupStatusTitle` extracts strings, not the run-outcome model, so the success/partial/failed classification still exists twice.** (P2, detail: `03_notify_tests_docker.md` §P2-C)
`internal/notify.go:31-40` and the log switch in `runProviderBackups` (`internal/backup.go:76-83`) classify the same `(succeeded, failed)` pair with differently phrased conditions. They agree, but nothing makes them agree. A small `backupOutcome` enum with `classifyBackups`, `title()` and `logMessage()` would make that one decision, and it would let `notify` classify once instead of each sender re-deriving it. While the titles are centralised, remove the stray leading U+FE0F bytes in `titleBackupsErrors` and `titleBackupsFailed` (confirmed with `od -c`). They are invisible and modify nothing.
Consequence: —
Fix: —

### Item 7
Location: internal/backup.go:203-207
Claim: **7. The centralised startup-log helper now holds the only copy of the "Organistations" typo, next to a redundant lower-casing.** (P3, detail: `01_provider_config.md` §P3-A)
`logProviderOrgs` (`internal/backup.go:203-207`) logs `"%s Organistations: %s"`, while its own doc comment spells the word correctly. It also calls `strings.ToLower` in the emptiness guard, where it cannot change the result. Fixing the typo is now a one-character change. Do it here together with the `"Gitlab"` label, and note the log-text change in the PR body instead of preserving known-wrong output.
Consequence: —
Fix: —

### Item 8
Location: internal/backup.go:443-471
Claim: **8. `validateStartupConfig` reads `GIT_BACKUP_DIR` differently from the rest of the code.** (P3, detail: `02_run_orchestration.md` §P3-B)
The newly extracted validator (`internal/backup.go:443-471`) uses `os.LookupEnv(envGitBackupDir)`. `displayStartupConfig` and `runProviderBackups` use `GetEnvOrFile`, which also honours `GIT_BACKUP_DIR_FILE`. This is inherited from base, but now that validation is its own function, the difference is easy to fix or to document on purpose.
Consequence: —
Fix: —

### Item 9
Location: internal/backup_test.go:621-637
Claim: **9. `TestGiteaOrgsRepositoryBackup` switches on its own loop literals and resets backups twice per iteration.** (P3, detail: `03_notify_tests_docker.md` §P3-C)
`internal/backup_test.go:621-637` loops over `[]string{sobaOrgTwo, "*"}` and then `switch`es on the same value. Each case calls `resetBackups()`, and the loop body calls it again. The two new assertion helpers (lines 651-681) rebuild the same `path.Join(...)` several times and repeat the org-two checks word for word. A table of `{orgs, expected dir → prefixes}` with one `assertGiteaOrgBackups` helper removes the switch, both bespoke helpers and the redundant resets.
Consequence: —
Fix: —

### Item 10
Location: docker/Dockerfile:8
Claim: **10. The merged Dockerfile `RUN` still ends in a no-op cleanup.** (P3, detail: `03_notify_tests_docker.md` §P3-D)
`rm -f "/var/cache/apk/*"` (`docker/Dockerfile:8`) quotes the glob, so it never expands, and `apk add --no-cache` leaves nothing to clean anyway. The PR edited this exact instruction, so delete the line.
Consequence: —
Fix: —
