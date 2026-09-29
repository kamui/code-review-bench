# Detail 01 — internal/backup.go (and internal/backup_test.go)

Scope: `git diff main...review-head -- internal/backup.go internal/backup_test.go`. backup.go stays at 774 lines (no 1k-line crossing). Commands used: `git diff main...review-head`, `grep -n` for anchors, `sed -n` for reading head. Tests were not executed (live provider tests skip without credentials; the diff was verified by reading).

## Behaviour-preservation check (verified by reading)

- Provider run order in `collectProviderBackupResults` (backup.go:97) matches the original: Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut. Verified.
- Startup-config log lines and their order per provider match the original for GitHub, Gitea, GitLab, Bitbucket, Azure. The odd `"Gitlab backup LFS: true"` capitalisation is preserved by passing `"Gitlab"` as the label at the call site. Verified.
- `checkProvidersDefined` (backup.go:680) now computes `bitbucketAPITokenComplete` before the map loop. The original computed it lazily inside a randomly-ordered map iteration. The final result only tests `count == 0`, so the outcome is the same; the change is a silent determinism fix, not "unchanged behaviour" as the PR body claims. Verified by reading.

## Finding A — Startup-config helpers are the same function five times (missed data-driven collapse)

Evidence: backup.go:233–292, `displayGitHubStartupConfig`, `displayGiteaStartupConfig`, `displayGitLabStartupConfig`, `displayBitBucketStartupConfig`, `displayAzureDevOpsStartupConfig`. Each is "return unless token env set; call some subset of logProviderOrgs / logProviderBackupsToKeep / logProviderCompareMethod / logProviderBackupLFS with (label, envVar)". Four `logProvider*` helpers each take a `(label, envVar)` pair. The provider metadata (label, gate env var, orgs/backups/compare/LFS env vars) is now spread across nine functions and repeated at every call site.

Why it matters: the refactor satisfies the Sonar cognitive-complexity counter by moving branches into helpers, but it does not reduce the number of concepts. Adding a provider still means writing a new `displayXStartupConfig`, adding it to `displayStartupConfig`, and remembering which optional lines apply. The `(label, envVar)` string parameters are stringly-typed and were already the source of the "Gitlab" vs "GitLab" inconsistency.

Code-judo proposal: define one table, e.g.

```go
type providerStartupInfo struct {
    label, gateEnv, orgsEnv, backupsEnv, compareEnv, lfsEnv string // empty = not applicable
}
```

with a single `logProviderStartupConfig(p providerStartupInfo)` that skips empty fields. GitHub's `skipUserRepos` and GitLab's min-access-level are the only special lines and can stay as two short explicit blocks after the generic call. Five functions and four helpers become one function and one slice. The same table could carry the token env var used by `collectProviderBackupResults`.

## Finding B — A third parallel provider registry was added instead of reusing the existing ones

Evidence: backup.go:105 defines a local `tokenProviders` slice of `{envVar, run}`. `constants.go` already has `enabledProviderAuth`, `justTokenProviders`, `userAndPasswordProviders` (used by `checkProvider`), and the display helpers above encode a fourth list of gate env vars. The Bitbucket "is configured?" test now has a shared helper (`bitbucketAPITokenDefined`/`bitbucketOAuthDefined`), but nothing else is shared.

Why it matters: "which providers exist and how do I detect that one is configured" is now answered in four places. Adding a provider (e.g. a new host) requires edits in each and there is no test that keeps them in sync.

Remedy: one provider descriptor (label, gate env var, run func, startup-log fields, auth params) in constants.go or a new `providers.go`, consumed by collect, display, and validation.

## Finding C — `checkProvidersDefined` keeps Bitbucket special-casing and a pointless tally

Evidence: backup.go:680–715. The function only ever asks whether the total count is zero, and `checkProvider` likewise only returns a count that is compared with zero. The Bitbucket switch arms still preserve an order-dependent "don't double-count OAuth if API token is complete" rule that has no observable effect. `bitbucketOAuthDefined()` is called inside the loop even though only `apiTokenComplete || oauthComplete` matters.

Code-judo: replace the count with a `bool anyDefined`, handle Bitbucket once before the loop (`if bitbucketAPITokenDefined() || bitbucketOAuthDefined() { anyDefined = true }`, exactly the expression already used in `collectProviderBackupResults`), and skip the two Bitbucket provider names in the loop. That deletes the switch, the eager `bitbucketAPITokenComplete` variable, and the count-dedup comment, and it stops relying on map iteration order being irrelevant. Same for `checkProvider` returning `(int, error)`.

## Finding D — `checkJustTokenProvider` / `checkUserAndPasswordProvider` use a `*strings.Builder` out-param and duplicate env-lookup logic

Evidence: backup.go:348–395. Both helpers look up `GetEnvOrFile(param)` and test `strings.Trim(val, " ") == ""`; the user/password one does the lookup twice per param (count pass, then error pass). The error text `"%s parameter '%s' is not defined.\n"` is formatted in three places. The builder is threaded in as a mutable pointer, and `checkProvider` (backup.go:326) still uses two independent `if slices.Contains` blocks with `count +=` on a zero value.

Remedy: add `func paramValue(param string) (val string, set bool)` returning trimmed non-empty status, and have each helper return `(count int, missing []string)`; `checkProvider` builds the error once from `missing`. Convert the two `if`s to a `switch`. This shrinks both helpers to a few lines and removes the out-param.

## Finding E — `runScheduledJob` hides a blocking call and writes a package global

Evidence: backup.go:525–545 assigns the package-level `job` (declared at backup.go:737, read by `execProviderBackups` at line 35 to decide whether to `os.Exit(1)`), then `s.Start()` and blocks in `waitForShutdown`. The name suggests registration only. The two `scheduleBackups` arms (backup.go:500–515) differ only in the log line and job definition/options; the `default` arm creates a scheduler that is never used or shut down (pre-existing, but the refactor left `gocron.NewScheduler()` above the switch instead of moving it into the scheduled arms).

Remedy: create the scheduler only when an interval or cron is configured; have `scheduleBackups` compute `(definition, options)` in a switch and call one `startScheduler` once; make `job` a return value or a field of a small struct rather than a mutated global whose nil-ness controls process exit. The one-shot path can then return `failed` directly to `Run` instead of calling `os.Exit` from inside `execProviderBackups` on a global's state.

## Finding F — Typo `"Organistations"` was extracted into a shared helper and kept

Evidence: backup.go:205, `logger.Printf("%s Organistations: %s", ...)`. Previously three copies of the typo, now one — the exact moment to fix it. The Copilot review also flagged it. Because the helper is now a single line, fixing it is a one-word change; leaving it means every provider's log line stays misspelled. If log-scrapers depend on the string it should be a named constant with a comment saying so, not an accidental misspelling.

## Finding G — Test helper split leaves redundant `resetBackups()` calls and near-duplicate assertion helpers

Evidence: backup_test.go:625–636 calls `resetBackups()` inside each switch arm and again after the switch, so it runs twice per iteration (pre-existing, but the refactor rewrote this block and kept it). `assertGiteaOrgTwoOnlyBackedUp` and `assertGiteaAllOrgsBackedUp` (backup_test.go:651–681) repeat the same `path.Join(os.Getenv(envGitBackupDir), "gitea.lessknown.co.uk", ...)` five times and both hand-check the same two org-two prefixes.

Code-judo: a single table-driven loop — `{org, wantRepos map[string][]string}` — with one `assertOrgRepos(t, org, prefixes)` helper and a single `resetBackups()` after each iteration replaces the switch and both assertion functions. It also removes `dirHasEntryWithPrefix`'s hand-rolled loop in favour of a check that yields a useful failure message (currently `require.True(t, false)`). The duplicate-test removal (`TestPublicGitLabRepositoryBackup2`) is correct and verified identical.

## Finding H — `Run()` decomposition moves code but leaves an ordering-coupled sequence

Evidence: backup.go:395–425 and helpers 430–490. `validateStartupConfig` both validates and returns the trimmed backup dir (mixing "check" and "compute"); `createWorkingDir` recomputes `GIT_WORKING_DIR` default logic that `resolveWorkingDir(backupDir)` (backup.go:~140) already implements. Verified in the diff: `createWorkingDir` still reads `os.Getenv(envGitWorkingDir)` and applies its own default rather than calling `resolveWorkingDir`.

Remedy: reuse `resolveWorkingDir` in `createWorkingDir` (canonical helper), and let `validateStartupConfig` return an error only; have the caller read/trim the directory once.
