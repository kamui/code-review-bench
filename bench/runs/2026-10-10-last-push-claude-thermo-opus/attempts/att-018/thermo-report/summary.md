# Thermo-nuclear code quality review — jonhadfield/soba#195

"refactor: resolve SonarQube findings" — range `c77f548..136a485` (`main...review-head`), 4 files, +312 / −305.

Reviewed in one primary context with `claude-opus-5-5` at `high`. No child reviewers, no cross-model or alternate-model review.

## Verdict

The PR does what it says on the behavioural side. I ran a base-versus-head differential over 4,000 pseudo-random environment permutations (set, blank, unset, dangling `_FILE`) through `displayStartupConfig`, `checkProvidersDefined` and `checkProvider` for all seven provider names, and the captured log output, return values and error text are byte-identical between `main` and `review-head`. The package test suite passes offline on the head (`ok github.com/jonhadfield/soba/internal 5.249s`; live-provider tests skip), `go vet ./...` is clean, and no file crosses the 1,000-line boundary — `backup_test.go` actually drops from 1,016 to 989 lines, and `GetEnvOrFile` call sites in `backup.go` fall from 45 to 24.

On the structural side the result is mixed, and under this review's bar it does not clear. Where the PR found a real parameter — the four `logProvider*` helpers, `backupStatusTitle`, `dirHasEntryWithPrefix` — it deleted repetition and the code is better. Elsewhere it satisfied the per-function complexity metric by slicing long functions into shorter ones without removing a single concept: `backup.go` goes from 23 functions to 42 and from 729 to 774 lines, and the reader now holds more names for the same amount of logic. Two of the extractions also leave a duplicate of something the file already had, and one adds a new local table whose name collides with an existing package-level list of different membership. None of this is a correctness problem; all of it is the kind of "moved, not deleted" outcome that a metric-driven refactor tends to produce and that a second pass should finish.

To check that the criticism is constructive rather than aesthetic, I applied the in-scope proposals for F1, F2, F3 and F4 together to a scratch copy of the head. It builds, is `gofmt`- and `go vet`-clean, passes the package tests, produces byte-identical output on the same 4,000-case differential for the startup display and provider validation, and brings `backup.go` back from 774 to 730 lines with `checkProvider`, `runScheduledJob` and the `userAndPasswordProviders` list deleted.

The PR is already merged, so the remediation below is written as a follow-up. Findings F1–F3 are the ones I would not have approved past; F4–F7 are worth doing in the same follow-up but are not blocking on their own.

## Findings

### F1 — "Is this provider configured?" gains a fourth encoding instead of losing three

**Anchor:** `internal/backup.go:97-138` (also `:233-294`, `internal/constants.go:120-159`)

`collectProviderBackupResults` introduces a function-local table called `tokenProviders` that pairs a gate environment variable with a provider entry point. The package already has `justTokenProviders` in `constants.go`, and the two disagree: the existing list is GitHub, GitLab, Gitea, Sourcehut, while the new table adds Azure DevOps and gates it on `envAzureDevOpsUserName`, which is a username and not a token. A reader who meets both names will reasonably assume they are the same set. Next to the table, `bitbucketAPITokenDefined` and `bitbucketOAuthDefined` hard-code the exact variable lists that `enabledProviderAuth[providerNameBitBucketAPIToken]` and `[providerNameBitBucketOAuth]` already declare, and the five new `display*StartupConfig` functions each open with their own hand-written copy of the gate. After this PR the question "is provider X on?" is answered independently by the new table, by five display guards, by `enabledProviderAuth` plus two membership lists in the validation path, and by each provider's own entry function — and the answers do not agree. I measured two of the disagreements on the head: with complete Bitbucket OAuth credentials and no `BITBUCKET_EMAIL`, the startup block printed zero Bitbucket lines in all 128 such cases, because the display guard looks only at the e-mail variable; and `GITEA_APIURL` alone was counted as a defined provider in 759 cases although the run table gates on `GITEA_TOKEN`. Both predate the PR, but the PR is the moment the gates were being lifted out and named, and it named them separately rather than once.

The remedy has an in-scope half and a follow-up half. In scope and behaviour-identical (confirmed on the scratch copy): `GetEnvOrFile` returns `""` on every path where it returns `false`, so the `exists` half of the thirteen `exists && val != ""` style predicates over it in this file is dead; a two-line `envSet(name) bool` plus an `envAllSet(names...)` replaces the eight of them that are pure provider gates (eleven lookups), lets the two Bitbucket helpers read their variable lists from `enabledProviderAuth` instead of restating them, and lets the local table be renamed to something that does not shadow `justTokenProviders` in the reader's head. The follow-up is the code-judo move: one package-level provider registry (`label`, `enabled`, `run`, `startup`) that both `collectProviderBackupResults` and `displayStartupConfig` range over, at which point the five display guards and the local table disappear. That step is not log-neutral — startup display order and backup run order differ today, Sourcehut has no startup block, and the Bitbucket display gate would change — so it should be its own PR that says so. Full evidence and worked code are in `01_provider_configuration.md`.

### F2 — The `checkProvider` split threads a counter nobody reads and now contradicts its own doc comment

**Anchor:** `internal/backup.go:322-395` (consumer at `:680-717`)

`checkProvider` was split into `checkJustTokenProvider` and `checkUserAndPasswordProvider`, each taking a `*strings.Builder` out-parameter and returning an `int` that the parent sums. The parent's comment still promises "the number of fully-configured provider entries it found (0 or 1)", while the new helper's comment correctly says it "counts the provider's non-empty auth parameters". The helper is right and the parent is wrong: on the head `checkProvider("Gitea")` returned 2 in 625 of 4,000 cases and `checkProvider("Sourcehut")` returned 2 in 587. More importantly, the only consumer of the number is `if count == 0` at line 712. The integer is plumbed through four functions, the hoisted `bitbucketAPITokenComplete` variable and the "only count if the API token method isn't already complete" guard exist purely to avoid double-counting, and none of that precision is ever observed. The same message also takes a detour: the helpers write into `checkProvider`'s builder, that is wrapped with `errors.New`, and `checkProvidersDefined` immediately unwraps it with `err.Error()` into a second builder and wraps it again. Finally, both Bitbucket names sit in `userAndPasswordProviders`, but `checkProvidersDefined` intercepts them in its `switch` before the default branch, so the new `checkUserAndPasswordProvider` is unreachable for them in production while the new `bitbucket*Defined` helpers implement the same "all parameters set" rule with different whitespace handling.

This is a refactor that moved the complexity into two helpers without questioning the contract. The cleaner shape is to make the contract a `bool` ("this provider is configured"), let `checkProvidersDefined` own the single error builder and pass it down, resolve Bitbucket before the map loop, and delete `checkProvider`, the counter and the double-count guard outright. Error text stays identical, which the scratch differential confirmed. At minimum, fix the "(0 or 1)" comment, which the PR's own new comment now disproves. Worked proposal in `01_provider_configuration.md`.

### F3 — `createWorkingDir` is a second copy of `resolveWorkingDir`, and `Run`'s other extracted steps are misnamed for what they do

**Anchor:** `internal/backup.go:473-487` (canonical helper at `:140-146`; siblings at `:426-471`)

The PR lifts the working-directory block out of `Run` into a new function `createWorkingDir`, whose first four lines read `GIT_WORKING_DIR` and fall back to `filepath.Join(backupDIR, workingDIRName)`. That is line-for-line what `resolveWorkingDir` does 330 lines higher in the same file, where the backup run and the cleanup path already use it. The duplication was inline before; extracting it into a named function without routing it through the existing helper turns an accident into an API, and leaves two places to change if the default ever moves. The body should be `workingDIR := resolveWorkingDir(backupDIR)` followed by the log line and the `MkdirAll`.

The neighbouring extractions have a naming problem of the same family. `logRequestTimeout` returns an `error` because it is in fact the validation gate for `GIT_REQUEST_TIMEOUT`, and `validateStartupConfig` returns the normalised backup directory because it is in fact also the resolver for it — one that reads `GIT_BACKUP_DIR` with `os.LookupEnv` and a newline trim, while `runProviderBackups` and `displayStartupConfig` read the same setting through `GetEnvOrFile`. `Run` now reads as four tidy calls, but the returned directory travels one hop into `createWorkingDir` and is then dropped, after which the backup run resolves it again by the other rule. The decomposition is a fair mechanical cut; it would be a good one if the timeout check lived inside the validation step and the backup directory had one accessor. Details in `02_run_orchestration.md`.

### F4 — `scheduleBackups` plus `runScheduledJob` is two functions where one flat one is simpler

**Anchor:** `internal/backup.go:491-541`

`runScheduledJob` was extracted to de-duplicate the register/start/wait tail of the interval and cron branches, which is the right instinct, but the result is a `switch` in which two arms `return runScheduledJob(...)` and the third falls out to a trailing `return nil`, a helper that needs an otherwise pointless `var err error` so it can assign the package-level `job`, and a singleton-mode option still spelled out in both arms. If the `switch` only chooses the job definition and its extra options, and the one-shot arm returns early, the shared tail can sit once at the bottom of `scheduleBackups` itself: the helper, the variadic pass-through, the mixed return style and the repeated option all go away, and the function stays well under the complexity threshold. Worked code is in `02_run_orchestration.md`; it compiles and the package tests pass with it, but the interval and cron arms block on a signal and were not exercised.

### F5 — The status-title extraction stops one copy short and adds three single-use constants

**Anchor:** `internal/notify.go:26-40` (remaining copy at `internal/backup.go:76-83`)

`backupStatusTitle` correctly collapses three identical `switch` statements. Two things are left over. First, once the helper exists, `titleBackupsSucceeded`, `titleBackupsErrors` and `titleBackupsFailed` are each referenced exactly once, inside that helper; they answer the duplicated-literal rule but are now pure indirection, parked in a `const` block that otherwise holds environment-variable names. Second, the same three-way classification of `(succeeded, failed)` is still written a fourth time in `runProviderBackups`, with differently shaped conditions (`succeeded == 0 && failed >= 0`, where the second clause is always true) that happen to partition the space identically. The honest model is a small typed status computed once from `getBackupsStats`, with a title and a log line hanging off it, passed to `notify` instead of two integers that every sender re-classifies. That deletes the constants, the fourth switch, and the tautological condition. Sketch in `03_notify_tests_docker.md`.

### F6 — The Gitea test helpers are two near-copies; the case table is the missing abstraction

**Anchor:** `internal/backup_test.go:625-681`

`assertGiteaOrgTwoOnlyBackedUp` and `assertGiteaAllOrgsBackedUp` share most of their bodies, rebuild `path.Join(os.Getenv(envGitBackupDir), "gitea.lessknown.co.uk", ...)` seven times between them, and are dispatched from a `switch org` inside a loop over the same two literals — so the loop and the switch encode the case list twice. The loop also still calls `resetBackups()` in each arm and again immediately after the switch. Putting the expectation in the loop's own table (organisation filter → expected directory → expected repository prefixes, with an empty expectation meaning "directory must not exist") leaves one small `assertGiteaOrgBackup(t, org, prefixes)` helper, removes the switch and both bespoke helpers, and reduces the resets to one. This test needs live Gitea credentials and skips here, so the proposal in `03_notify_tests_docker.md` is unexecuted.

### F7 — A behaviour-preserving refactor of five functions shipped with no credential-free test pinning the behaviour

**Anchor:** `internal/backup.go:190-294` (also `:326-395`, `:680-717`)

The PR body rests on "behaviour and log output are unchanged" and "the full test suite passes locally", but the only test in the package that reaches any of the refactored configuration code without provider credentials is `TestCheckProvidersFailureWhenNoneDefined`, which covers the empty-environment path. `displayStartupConfig`, `checkProvider` and the Bitbucket predicates are otherwise exercised only by live tests that skip without tokens. The claim turned out to be true — the differential described above found no difference — but nothing in the repository would have caught it had a label or a gate been transposed, and the preserved quirks (`"Gitlab backup LFS"` beside `"GitLab backups to keep"`, the `Organistations` spelling now living in a single format string) are exactly the sort of thing the next tidy-up will "fix" unknowingly. A table-driven test that sets a handful of environments, swaps `logger` for a buffer and asserts the startup lines and the `checkProvidersDefined` result would cost about forty lines and make the next refactor of this file safe. The harness I used is described in `01_provider_configuration.md` and can be cut down into that test.

## Things checked and found sound

The Dockerfile changes are mechanical and correct as far as they can be judged without a build: the two `RUN` layers are merged, the package list is sorted, and the release URL is quoted. The pre-existing `rm -f "/var/cache/apk/*"` is a no-op because the glob is quoted (and redundant with `--no-cache`); that is not this PR's doing. Removing `TestPublicGitLabRepositoryBackup2` is correct — its body is identical to `TestPublicGitLabRepositoryBackup`. Provider run order and every startup log line and its position are preserved. The only behavioural delta I could find by reading is cosmetic: with complete Bitbucket API-token credentials and a dangling `BITBUCKET_USER_FILE`-style path, the short-circuit in `collectProviderBackupResults` logs the "file does not exist" diagnostic once per run instead of twice.

## Questions for the author

### Q1 — Is the Bitbucket startup block meant to be gated on `BITBUCKET_EMAIL` alone?

**Anchor:** `internal/backup.go:276-284`

`displayBitBucketStartupConfig` prints nothing for an OAuth-only Bitbucket configuration, even though that configuration validates and runs. If that is unintended, the guard should become `bitbucketAPITokenDefined() || bitbucketOAuthDefined()`, which this PR has just made available; if it is intended, a comment should say why.

### Q2 — Should `GITEA_APIURL` or `SOURCEHUT_APIURL` on its own satisfy "a provider is defined"?

**Anchor:** `internal/backup.go:348-365`

`checkJustTokenProvider` counts every non-blank entry in `enabledProviderAuth[provider]`, and for Gitea and Sourcehut that list includes the API URL. With only the URL set, start-up validation passes and the run then backs up nothing and logs "all backups failed". If the URL is meant to be optional configuration rather than a credential, it probably does not belong in the auth list.

## Proposed remediation sequence

1. Reuse `resolveWorkingDir` in `createWorkingDir`, and correct the `checkProvider` "(0 or 1)" comment. Both are one-line changes with no behavioural surface (F3, F2).
2. Add the credential-free characterisation test for the startup log and `checkProvidersDefined` before touching anything else in this file (F7).
3. Introduce `envSet` / `envAllSet`, derive the Bitbucket predicates from `enabledProviderAuth`, and rename the local `tokenProviders` table (F1, in-scope half).
4. Change the provider check contract from a count to a `bool` with a single error builder, and delete `checkProvider` and the double-count guard (F2).
5. Flatten `scheduleBackups` and drop `runScheduledJob`; fold the timeout check into the validation step (F4, F3).
6. Replace the status integers with a typed status and table-drive the Gitea assertions (F5, F6).
7. As a separate PR that announces its log changes, introduce the provider registry and resolve Q1 and Q2 through it (F1, follow-up half).

## Detail files

- `01_provider_configuration.md` — F1, F2, F7; the differential harness, its commands and measurements; worked proposals for `envSet`, the bool contract and the registry.
- `02_run_orchestration.md` — F3, F4; side-by-side of the duplicated resolver; worked flat `scheduleBackups`.
- `03_notify_tests_docker.md` — F5, F6; status-type sketch, table-driven Gitea test, Dockerfile and file-size notes.
