# 01 — `internal/backup.go`: provider selection, startup config, validation, scheduling

Scope: every hunk of `internal/backup.go` in `main...review-head` (commit `136a485`).
File size: 729 lines at the merge-base, 774 lines at head (+45). Well under the 1k threshold; no file-size concern.

## Commands run

```
git diff main...review-head -- internal/backup.go
git show main:internal/backup.go | wc -l          # 729
wc -l internal/backup.go                          # 774
grep -n -e checkProvider( -e resolveWorkingDir -e createWorkingDir -e "job ==" -e "job !=" internal/*.go
go vet ./...                                      # clean
go test ./internal/ -count=1                      # ok (live provider tests skip, no credentials)
go test ./internal/ -count=1 -coverprofile=...    # per-function coverage quoted below
```

All commands were run from the clone root with the offline Go environment prescribed by the run policy. The clone was left unmodified (`git status --short` empty afterwards).

---

## Finding 1.1 — Provider knowledge is now encoded in five parallel places; the PR added one more instead of a registry

**Where.** `internal/backup.go:97-122` (`collectProviderBackupResults` and its anonymous `tokenProviders` table), `internal/backup.go:125-138` (Bitbucket predicates), `internal/backup.go:190-294` (`displayStartupConfig` plus five `displayXStartupConfig` functions and four `logProvider*` helpers), `internal/backup.go:680-717` (`checkProvidersDefined`), and `internal/constants.go:120-159` (`enabledProviderAuth`, `justTokenProviders`, `userAndPasswordProviders`).

**Evidence.** After this PR, the question "which providers exist, how is each one switched on, and which env vars configure it" is answered independently by:

1. `collectProviderBackupResults` — a new, function-local anonymous struct slice `{envVar, run}` for five providers, plus a hand-coded Bitbucket branch above it.
2. The five `displayXStartupConfig` functions — each re-derives an "enabled" gate and hard-codes its own label and env-var names.
3. `enabledProviderAuth` — the auth parameter map in `constants.go`.
4. `justTokenProviders` / `userAndPasswordProviders` — two list-membership tables that drive `checkProvider`.
5. `checkProvidersDefined` — a switch that special-cases the two Bitbucket entries before falling back to `checkProvider`.

The five encodings do not agree with each other, and nothing forces them to:

- **Run gate vs. validation gate, Gitea.** Validation (`checkJustTokenProvider`, via `enabledProviderAuth[Gitea] = {GITEA_API_URL, GITEA_TOKEN}`) counts any non-blank parameter, so `GITEA_API_URL` alone makes `checkProvidersDefined` succeed. The run gate in `tokenProviders` needs `GITEA_TOKEN`. With only the URL set, startup passes validation and then every run backs up nothing and logs "all backups failed".
- **Run gate vs. validation gate, Azure DevOps.** Validation needs both `AZURE_DEVOPS_USERNAME` and `AZURE_DEVOPS_PAT` (user-and-password policy). The run gate in `tokenProviders` keys on the *username* only (`{envAzureDevOpsUserName, AzureDevOps}`).
- **Display gate vs. run gate, Bitbucket.** `displayBitBucketStartupConfig` (`backup.go:276-284`) gates on `envBitBucketEmail` only, so an OAuth-only Bitbucket user gets no startup config lines, even though `collectProviderBackupResults` runs Bitbucket for them. The PR *created* `bitbucketAPITokenDefined()`/`bitbucketOAuthDefined()` two hundred lines above and did not use them here.
- **Ordering.** Display order is GitHub, Gitea, GitLab, BitBucket, Azure DevOps. Run order is Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut. Sourcehut has no startup display at all.

These inconsistencies were there before the PR, so none of them is a regression. The structural problem is that the PR broke up `runProviderBackups` and `displayStartupConfig` "for cognitive complexity" and did it by adding one more parallel table (the anonymous `tokenProviders` struct) and six more free functions, rather than introducing the single model that all five sites are missing. A refactor aimed at lowering the number of concepts a reader must hold ended up with more places to update when a provider is added.

**Code-judo proposal.** Introduce one provider descriptor and make every site a loop over it:

```go
// providers.go
type providerSpec struct {
	label      string                                  // "GitHub", "Azure DevOps", ...
	enabled    func() bool                             // single source of truth for "is this provider on"
	run        func(backupDir string) *ProviderBackupResults
	orgsEnv    string                                  // "" when not applicable
	backupsEnv string
	compareEnv string
	lfsEnv     string
	extraLog   func()                                  // GitHub skip-user-repos, GitLab min access level
}

var providers = []providerSpec{
	{label: "BitBucket", enabled: func() bool { return bitbucketAPITokenDefined() || bitbucketOAuthDefined() },
		run: Bitbucket, backupsEnv: envBitBucketBackups, compareEnv: envBitBucketCompare, lfsEnv: envBitBucketBackupLFS},
	{label: "Gitea", enabled: envSet(envGiteaToken), run: Gitea,
		orgsEnv: envGiteaOrgs, backupsEnv: envGiteaBackups, compareEnv: envGiteaCompare, lfsEnv: envGiteaBackupLFS},
	{label: "GitHub", enabled: envSet(envGitHubToken), run: GitHub,
		orgsEnv: envGitHubOrgs, compareEnv: envGitHubCompare, lfsEnv: envGitHubBackupLFS, extraLog: logGitHubSkipUserRepos},
	// GitLab, Azure DevOps, Sourcehut ...
}

func collectProviderBackupResults(backupDir string) []ProviderBackupResults {
	var results []ProviderBackupResults
	for _, p := range providers {
		if p.enabled() {
			results = append(results, *p.run(backupDir))
		}
	}
	return results
}

func displayStartupConfig() {
	// backup-dir line as today ...
	for _, p := range providers {
		if !p.enabled() {
			continue
		}
		if p.extraLog != nil { p.extraLog() }
		if p.orgsEnv != ""    { logProviderOrgs(p.label, p.orgsEnv) }
		if p.backupsEnv != "" { logProviderBackupsToKeep(p.label, p.backupsEnv) }
		if p.compareEnv != "" { logProviderCompareMethod(p.label, p.compareEnv) }
		if p.lfsEnv != ""     { logProviderBackupLFS(p.label, p.lfsEnv) }
	}
}
```

This deletes the five `displayXStartupConfig` functions, the anonymous `tokenProviders` table and the Bitbucket special case in `collectProviderBackupResults`. Adding a provider becomes one table row. `checkProvidersDefined` then becomes "any `p.enabled()`, plus per-provider partial-config errors", and the partial-config policy can hang off the same spec (see Finding 1.3), which retires `justTokenProviders`/`userAndPasswordProviders` as separate tables.

Behaviour caveats the author must choose on deliberately: (a) using one `enabled` predicate for both display and run changes the Bitbucket OAuth-only startup log (it starts printing config lines) and the Azure DevOps run gate (if the spec uses the validation predicate). Both are arguably bug fixes, but they are changes; (b) the table's order sets the order of results in notifications and webhooks, so it should follow today's run order, not today's display order; (c) GitLab's LFS label is `"Gitlab"` today (see summary finding 8).

**Verification status.** The divergences are confirmed by reading `constants.go:120-159` against `backup.go:97-122` and `backup.go:276-284`. The Gitea URL-only path was traced by hand: `checkJustTokenProvider` returns 1 for a non-blank `GITEA_API_URL`, and `tokenProviders` requires `GITEA_TOKEN`. It was not executed end-to-end because live provider calls are unavailable. The proposal is a design sketch and was not compiled.

---

## Finding 1.2 — `createWorkingDir` re-implements the existing `resolveWorkingDir`

**Where.** `internal/backup.go:473-487` (new helper) vs `internal/backup.go:140-146` (existing canonical helper, used by `runProviderBackups` at line 52).

**Evidence.**

```go
// backup.go:140 (existing)
func resolveWorkingDir(backupDir string) string {
	if w := os.Getenv(envGitWorkingDir); w != "" {
		return w
	}
	return filepath.Join(backupDir, workingDIRName)
}

// backup.go:473 (extracted by this PR)
func createWorkingDir(backupDIR string) error {
	// Check if GIT_WORKING_DIR is set, otherwise use default
	workingDIR := os.Getenv(envGitWorkingDir)
	if workingDIR == "" {
		workingDIR = filepath.Join(backupDIR, workingDIRName)
	}
	...
```

The inline copy was already in `Run` before the PR. By extracting it into a named helper that sits next to `resolveWorkingDir`, the PR made the duplication permanent and easy to find, but did not remove it. If the resolution rule changes (for example trimming, `filepath.Clean`, or relative-path handling), startup would create one directory and the run would use and then clean up a different one. That is exactly the class of drift `cleanupWorkingDir`'s safety check was written to protect against.

**Remedy.** A one-line change: `workingDIR := resolveWorkingDir(backupDIR)`. At that point `createWorkingDir` is short enough that it could also go back inline into `Run` without complexity cost.

**Verification status.** Confirmed by reading. The coverage run shows `createWorkingDir` at 0.0%, so no offline test would catch a divergence.

---

## Finding 1.3 — The `checkProvider` split moves complexity around without deleting any

**Where.** `internal/backup.go:322-395` (`checkProvider`, `checkJustTokenProvider`, `checkUserAndPasswordProvider`), `internal/backup.go:680-717` (`checkProvidersDefined`), `internal/constants.go:149-159`.

**Evidence.**

- *Dispatch by parallel list membership.* `checkProvider` still decides policy with two `slices.Contains` calls over two separate global lists. A provider's auth policy is not attached to the provider.
- *Unreachable table entries.* `userAndPasswordProviders` contains `BitBucketAPIToken`, `BitBucketOAuth` and `AzureDevOps`. `checkProvidersDefined` intercepts both Bitbucket names in its own `switch`, so `checkUserAndPasswordProvider` only ever serves Azure DevOps. Two of its three table entries are dead for the only caller. This is also why Bitbucket partial-configuration errors (for example email set, token blank) are never reported, while Azure DevOps partial configuration is.
- *Pointless Bitbucket counting.* `count` in `checkProvidersDefined` is only ever compared with `0` (`backup.go:711`). The "only count OAuth if API token isn't already complete" dance (`backup.go:685-697`) therefore has no observable effect. It was order-dependent over Go's randomised map iteration before this PR (OAuth visited first → both counted), and the PR made it deterministic. The simpler move is to delete it: `if bitbucketAPITokenDefined() || bitbucketOAuthDefined() { anyDefined = true }` outside the loop.
- *Out-parameter error accumulation.* Both new helpers take `outputErrs *strings.Builder` and return an `int`, so each helper's contract is "return a count and also write into your caller's buffer". Returning `(ok bool, problems []string)` would make the boundary explicit and let `checkProvider` do the joining.
- *Double environment reads.* `checkUserAndPasswordProvider` walks the parameter list twice and calls `GetEnvOrFile` (which can read a file per call) twice per parameter.
- *Stale contract.* The doc on `checkProvider` (`backup.go:322-325`) still says it returns "0 or 1". The new doc on `checkJustTokenProvider` correctly says it "counts the provider's non-empty auth parameters", which is 2 for Gitea and Sourcehut when both URL and token are set. The PR wrote a comment that contradicts its parent's comment.

**Code-judo proposal.** Classify each parameter once, then apply the policy:

```go
type authPolicy int
const (
	authAnyToken authPolicy = iota // token providers: blank-but-present is an error
	authAllOrNothing               // user+password providers: partial is an error
)

func checkProvider(name string, params []string, policy authPolicy) (configured bool, problems []string) {
	var set, blank, absent []string
	for _, p := range params {
		v, ok := GetEnvOrFile(p)
		switch {
		case !ok:                           absent = append(absent, p)
		case strings.TrimSpace(v) == "":    blank = append(blank, p)
		default:                            set = append(set, p)
		}
	}
	switch policy {
	case authAnyToken:
		problems = blank
		configured = len(set) > 0
	case authAllOrNothing:
		if len(set) > 0 && len(set) < len(params) { problems = append(blank, absent...) }
		configured = len(set) == len(params)
	}
	return configured, problems
}
```

Hang `authPolicy` and `params` off the provider spec from Finding 1.1. `checkProvidersDefined` then becomes a flat loop with no `switch`, and `justTokenProviders`, `userAndPasswordProviders` and the Bitbucket special cases all disappear. Two caveats: `strings.TrimSpace` differs slightly from today's `strings.Trim(val, " ")` (tabs and newlines), so keep `Trim(val, " ")` if byte-for-byte parity matters; and error-message ordering should stay in `params` order.

**Verification status.** Confirmed by reading and by grep: `checkProvider(` has exactly one call site (`backup.go:699`). Coverage: `checkJustTokenProvider` 66.7%, `checkUserAndPasswordProvider` 57.1%. The proposal is a sketch and was not compiled.

---

## Finding 1.4 — `runScheduledJob` hides a package-global write, and run mode is signalled through `job == nil`

**Where.** `internal/backup.go:525-541` (`runScheduledJob`), `internal/backup.go:30-38` (`execProviderBackups`), `internal/backup.go:85-90`, `internal/backup.go:737` (`var job gocron.Job`).

**Evidence.** `runScheduledJob` declares `var err error` only so it can write the package-level `job` with `=` rather than shadow it with `:=`. Its doc comment ("registers the backup task with the scheduler, starts it and blocks until shutdown") does not mention that it also mutates global state. That global state changes the behaviour of `execProviderBackups`: `job == nil && failed > 0` → `os.Exit(1)`. So whether a failed run kills the process is decided by whether some other helper, two call levels away, happened to assign a global first. The pre-PR code had the same coupling, but it was at least visible in the body of `Run`. The refactor moved the write into a generically named helper, which makes it easier to miss.

**Remedy / code-judo.** Make the mode explicit at the call site so the nil check disappears:

```go
default: // one-shot
	if runProviderBackups() > 0 {
		os.Exit(1)
	}
```

and register `gocron.NewTask(func() { runProviderBackups() })` for the scheduled branches. `execProviderBackups` and its `job == nil` test can then be deleted. The "next run" log line still needs the job handle. At minimum, `runScheduledJob` should document that it sets `job`. Preferably it should return the `gocron.Job` and let `scheduleBackups` own the assignment.

**Verification status.** Confirmed by reading. `scheduleBackups` and `runScheduledJob` show 0.0% offline coverage, so neither the one-shot exit path nor the scheduled path is exercised by the suite.

---

## Non-findings checked

- `validateStartupConfig` flattening `if ghOrgsExists { if !githubTokenExists {...} }` into `&&` is equivalent.
- `logProviderCompareMethod` replaces Bitbucket's `strings.ToLower(x) == "refs"` with `strings.EqualFold`. This is equivalent for the ASCII constant. Printing `compareTypeClone`/`compareTypeRefs` produces the same strings as the previous literals.
- `logRequestTimeout` / `validateStartupConfig` / `createWorkingDir` preserve the original check order and error text.
- The `logProvider*` helpers are reasonable local extractions. The issue is the per-provider functions that call them (Finding 1.1), not the helpers themselves.
