# Review blind-dd405c

### Item 1
Location: internal/backup.go:97-294
Claim: **P1 — The refactor adds a sixth provider registry instead of collapsing the five that already exist.** In `internal/backup.go:97-137` and `internal/backup.go:190-294`, `collectProviderBackupResults` introduces an anonymous `tokenProviders` table plus a separate Bitbucket branch, and `displayStartupConfig` becomes five hand-written `display*StartupConfig` functions. These sit alongside `enabledProviderAuth`, `justTokenProviders` and `userAndPasswordProviders` in `constants.go:120-159` and the Bitbucket special-casing in `checkProvidersDefined`. Each of these places decides separately what "this provider is configured" means, and they disagree. Bitbucket display is gated on the email alone, so OAuth-only users get no startup config. Azure DevOps runs on the username alone but validation requires username and PAT. Sourcehut has no startup log at all. The complexity was moved, not deleted. The remedy is one `providerSpec` table (name, label, enabled predicate, run func, orgs/backups/compare/LFS env vars, optional extra logger). Backup collection, startup display and the "any provider defined" check then become three short loops over it, and all five `display*` functions plus the ad-hoc struct slice disappear. Keep today's gates per row first so the change stays behaviour-preserving, then unify them deliberately. Full evidence and a worked sketch are in `01_provider_config.md`.
Consequence: —
Fix: —

### Item 2
Location: internal/backup.go:680-717
Claim: **P2 — `checkProvidersDefined` keeps dead Bitbucket de-dup arithmetic, and the Bitbucket entries in `userAndPasswordProviders` are unreachable.** In `internal/backup.go:680-717`, `count` is only ever compared with zero, so the "only count OAuth if the API token isn't complete" rule cannot affect the outcome. The loop also intercepts both Bitbucket provider names before `checkProvider`, so the newly extracted `checkUserAndPasswordProvider` never runs for two of its three nominal providers, and partial Bitbucket configs never get the "parameter X is not defined" diagnostics that Azure DevOps gets. (The PR also silently made the old map-order-dependent count deterministic. That is harmless, because only `== 0` is observable.) Replace `count` with a boolean, collapse Bitbucket to `bitbucketAPITokenDefined() || bitbucketOAuthDefined()`, and either drop Bitbucket from `userAndPasswordProviders` or route it through `checkProvider` on purpose. Detail is in `01_provider_config.md`.
Consequence: —
Fix: —

### Item 3
Location: internal/backup.go:326-395
Claim: **P3 — `checkUserAndPasswordProvider` does two passes over the same env vars and returns a bool disguised as `int`.** In `internal/backup.go:326-395`, the helper counts found and total parameters, then re-reads and re-trims every parameter to find the missing ones, and returns a literal `1` or `0`. `checkJustTokenProvider` returns a parameter count that its only consumer treats as a boolean. A single pass that collects `missing []string` and returns `len(missing) == 0` removes the second loop and both counters, and `checkProvider`'s two independent `slices.Contains` checks become a single dispatch. Detail is in `01_provider_config.md`.
Consequence: —
Fix: —

### Item 4
Location: internal/backup.go:525-542
Claim: **S1 — `runScheduledJob` hides the write to the package-global `job` that controls one-shot versus scheduled behaviour.** In `internal/backup.go:525-542`, the helper declares `var err error` only so it can assign the global `job`. `execProviderBackups` (`backup.go:35`) and `runProviderBackups` (`backup.go:87`) read that global to decide whether to `os.Exit(1)` and whether to print the next-run banner. Before the PR, this assignment was visible in `Run` next to the one-shot `default:` branch. Now it is two calls deep in a helper whose name and comment don't mention it. At minimum, return the job or make the assignment explicit and commented, and fold the `WithSingletonMode` option that both call sites repeat into the helper. The better direction is to pass the scheduled/one-shot mode into `execProviderBackups` through the task closure, so the global stops being a mode flag. Detail is in `02_run_and_scheduler.md`.
Consequence: —
Fix: —

### Item 5
Location: internal/notify.go:31-40
Claim: **N1 — The same three-way outcome classification survives a fourth time in `runProviderBackups`, with differently written predicates.** `backupStatusTitle` in `internal/notify.go:31-40` and the log switch in `internal/backup.go:76-83` classify `(succeeded, failed)` into the same three states, but with a different case order and different defaults, so a reader has to prove by hand that they agree. The PR deduplicated three of the four copies. A small `backupOutcome` enum with `classifyBackups`, `title()` and `logLine()` finishes the job, and it lets the notification senders take the outcome instead of recomputing it from two ints. Detail is in `03_notify.md`.
Consequence: —
Fix: —

### Item 6
Location: internal/backup_test.go:622-681
Claim: **T1 — `TestGiteaOrgsRepositoryBackup` was split into two near-copy assertion helpers instead of being made table-driven, and it keeps the triple `resetBackups()`.** In `internal/backup_test.go:622-681`, `resetBackups()` still runs inside each `case` and again after the `switch` (plus a deferred call). `assertGiteaOrgTwoOnlyBackedUp` and `assertGiteaAllOrgsBackedUp` repeat the same `path.Join(..., "gitea.lessknown.co.uk", org)` six times, and the second is just the first plus one org. Encoding the expectation as data (filter → org → expected repo prefixes, plus absent orgs) removes the `switch`, both bespoke helpers and the redundant resets, and keeps `dirHasEntryWithPrefix`. Detail is in `04_tests_and_docker.md`.
Consequence: —
Fix: —

### Item 7
Location: internal/backup.go:203-273
Claim: **P4 — The new shared log helper now owns the "Organistations" typo and a `Gitlab`/`GitLab` label inconsistency.** At `internal/backup.go:203-207` and `internal/backup.go:273`, the PR centralised the organisations log line into `logProviderOrgs` and kept the misspelling, and `displayGitLabStartupConfig` passes `"Gitlab"` to one helper and `"GitLab"` to the others. Keeping the output unchanged was the stated goal, but now that each string exists once, fixing it is a one-line change. The P1 table would also make per-provider label drift impossible. Detail is in `01_provider_config.md`.
Consequence: —
Fix: —

### Item 8
Location: internal/notify.go:27-28
Claim: **N2 — The new title constants embed an invisible leading U+FE0F.** At `internal/notify.go:27-28`, `titleBackupsErrors` and `titleBackupsFailed` start with a stray variation selector (bytes `EF B8 8F`) before the emoji, copied from the old literals. Now that these are the named single source of truth, the invisible character will trip future string comparisons. Remove it, or comment why it is intentional. Detail is in `03_notify.md`.
Consequence: —
Fix: —

### Item 9
Location: docker/Dockerfile:5-8
Claim: **D1 — The rewritten Dockerfile `RUN` keeps a no-op cleanup step.** At `docker/Dockerfile:5-8`, `rm -f "/var/cache/apk/*"` quotes the glob, so it deletes nothing, and `apk add --no-cache` leaves nothing to delete anyway. The PR rewrote this exact instruction and carried the dead command forward. Delete it. Detail is in `04_tests_and_docker.md`.
Consequence: —
Fix: —

### Item 10
Location: internal/backup.go:680-717
Claim: **Q1.** Should partial Bitbucket credentials (for example user and key set but secret missing) produce the same "parameter X is not defined" validation error that Azure DevOps produces? Today they are silently ignored because `checkProvidersDefined` bypasses `checkProvider` for both Bitbucket methods. The answer decides whether P2's remedy drops Bitbucket from `userAndPasswordProviders` or routes it through the shared check.
Consequence: —
Fix: —
