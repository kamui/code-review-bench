# 01 — Provider model: enablement, startup display, credential validation

Scope: `internal/backup.go` lines 95–138 (`collectProviderBackupResults`, `bitbucket*Defined`),
190–294 (`displayStartupConfig` and its nine helpers), 322–395 (`checkProvider` and its two
helpers), 680–717 (`checkProvidersDefined`); with `internal/constants.go:120-159` and
`internal/bitbucket.go:15-25` as the surrounding canonical code. Covers summary findings F1, F2
and F6.

All line numbers are on the head, `136a4850`.

## Measurements

Commands were run from the clone root unless a scratch path is named.

| Measure | Base (`main`) | Head | Command |
| --- | --- | --- | --- |
| `internal/backup.go` lines | 729 | 774 | `git show main:internal/backup.go \| wc -l`; `wc -l internal/backup.go` |
| top-level funcs in `backup.go` | 23 | 42 | `grep -c '^func '` on each side |
| `GetEnvOrFile(` call sites in `backup.go` | 45 | 24 | `grep -c 'GetEnvOrFile('` on each side |
| diff total | — | +312 / −305 | `git diff --stat main...review-head` |

The call-site count halving is the PR's real gain: the four `logProvider*` helpers and the two
`bitbucket*Defined` predicates removed genuine copy-paste. The function count nearly doubling
while the line count rose by 45 is the cost: the file has more names to hold in one's head and no
fewer concepts.

## F1 — Five hand-synchronised descriptions of "a provider"

### Evidence

After the PR a provider is described in these places, none of which refers to another:

1. `enabledProviderAuth` — `constants.go:120-148`, a map from provider name to credential
   env vars.
2. `justTokenProviders` — `constants.go:149-154`.
3. `userAndPasswordProviders` — `constants.go:155-159`.
4. **New in this PR:** the anonymous `tokenProviders` slice inside
   `collectProviderBackupResults`, `backup.go:105-114`, mapping one env var to a run function,
   with BitBucket handled by a bespoke `if` above it (`backup.go:100-103`).
5. **New in this PR:** five functions `displayGitHubStartupConfig` … `displayAzureDevOpsStartupConfig`,
   `backup.go:233-294`, each opening with its own enablement guard.

Each provider's own entry point (`GitHub()`, `Bitbucket()`, …) then re-checks its credentials a
further time (`github.go:14-25`, `bitbucket.go:15-39`).

The new table's name contradicts the existing classification. `tokenProviders` contains
`{envAzureDevOpsUserName, AzureDevOps}` (`backup.go:112`), keyed on a *username*, while
`constants.go:155-159` files Azure DevOps under `userAndPasswordProviders`, not
`justTokenProviders`.

The enablement predicates disagree between the three flows. Read off the head:

| Provider | Runs a backup when (`backup.go:97-123`) | Startup config shown when (`backup.go:233-294`) | Counts as "defined" when (`backup.go:680-717`) |
| --- | --- | --- | --- |
| GitHub | `GITHUB_TOKEN` non-empty | same | `GITHUB_TOKEN` non-blank |
| GitLab | `GITLAB_TOKEN` non-empty | same | `GITLAB_TOKEN` non-blank |
| Gitea | `GITEA_TOKEN` non-empty | same | `GITEA_APIURL` **or** `GITEA_TOKEN` non-blank (each counted) |
| Sourcehut | `SOURCEHUT_PAT` non-empty | **never** (no display function) | `SOURCEHUT_APIURL` **or** `SOURCEHUT_PAT` non-blank |
| Azure DevOps | `AZURE_DEVOPS_USERNAME` non-empty | same | username **and** PAT non-blank |
| BitBucket | API-token pair complete **or** OAuth triple complete | `BITBUCKET_EMAIL` non-empty only (`backup.go:277`) | API pair complete, or OAuth triple complete |

These divergences all predate the PR. What the PR did is give each one its own named function,
which makes them permanent-looking, and add a fourth table rather than routing the three flows
through one.

### Verification

Run in a scratch copy of the head under `clone-work/scratch/soba` (the clone itself was not
modified; `git status --short` on the clone is empty before and after). Probe file
`internal/zz_probe_test.go`, executed with
`go test ./internal/ -count=1 -run TestProbe -v` under the offline Go environment.

```
checkProvider(Gitea) with APIURL+TOKEN -> count=2 err=<nil> (doc says 0 or 1)
only GITEA_APIURL set -> checkProvidersDefined err=<nil>; run gate (token set && non-empty)=false
only SOURCEHUT_APIURL set -> checkProvidersDefined err=<nil>
only AZURE_DEVOPS_USERNAME set -> checkProvidersDefined err="AzureDevOps parameter 'AZURE_DEVOPS_PAT' is not defined.\n"
```

So with only `GITEA_APIURL` (or only `SOURCEHUT_APIURL`) set, startup validation passes and the
run then backs up nothing — a direct product of validation and execution consulting different
tables. Status: **verified by execution** for the validation side; the run-gate side is read from
`backup.go:109,113,117` (executing it would start real backups).

The BitBucket display gate was verified by reading only: `backup.go:277` reads
`envBitBucketEmail` into a variable named `bbUser`, so a deployment configured with the OAuth
triple alone runs BitBucket backups but prints no BitBucket startup configuration.

### Worked code-judo proposal, stage A (behaviour-preserving, verified)

The five display functions differ only in which of four optional settings they print and in two
provider-specific lines. One fixed sequence — orgs, provider-specific extra, backups-to-keep,
compare method, LFS — reproduces every provider's existing line order:

```go
type providerDisplay struct {
	label      string // prefix of every startup log line
	enabledEnv string
	orgsEnv, backupsEnv, compareEnv, lfsEnv string // "" = setting is not displayed
	extra      func()
}

var providerDisplays = []providerDisplay{
	{label: "GitHub", enabledEnv: envGitHubToken, orgsEnv: envGitHubOrgs,
		compareEnv: envGitHubCompare, lfsEnv: envGitHubBackupLFS, extra: logGitHubSkipUserRepos},
	{label: "Gitea", enabledEnv: envGiteaToken, orgsEnv: envGiteaOrgs, backupsEnv: envGiteaBackups,
		compareEnv: envGiteaCompare, lfsEnv: envGiteaBackupLFS},
	{label: "GitLab", enabledEnv: envGitLabToken, backupsEnv: envGitLabBackups,
		compareEnv: envGitLabCompare, lfsEnv: envGitLabBackupLFS, extra: logGitLabMinAccessLevel},
	{label: "BitBucket", enabledEnv: envBitBucketEmail, backupsEnv: envBitBucketBackups,
		compareEnv: envBitBucketCompare, lfsEnv: envBitBucketBackupLFS},
	{label: "Azure DevOps", enabledEnv: envAzureDevOpsUserName, orgsEnv: envAzureDevOpsOrgs,
		compareEnv: envAzureDevOpsCompare, lfsEnv: envAzureDevOpsBackupLFS},
}

func displayStartupConfig() {
	if backupDIR, _ := GetEnvOrFile(envGitBackupDir); backupDIR != "" {
		logger.Printf("root backup directory: %s", backupDIR)
	}

	for _, p := range providerDisplays {
		p.display()
	}
}
```

with `display()` being the body of the four existing `logProvider*` helpers inlined in that fixed
order, each skipped when its env-var field is empty. That deletes five functions and four helpers
(nine names) in exchange for one type, one table, one method and two small hooks
(`logGitHubSkipUserRepos`, `logGitLabMinAccessLevel`) for the lines only one provider prints, and adding Sourcehut's missing
startup output becomes one table row.

Verification of stage A: the scratch probe `TestProbeDisplayEquivalence` implements exactly this
table (keeping the `Gitlab` LFS label as a per-row override) and compares its log output with the
head's `displayStartupConfig` over 20,000 pseudo-random environment combinations of all 24
relevant variables in unset / set-empty / set-value states:

```
display equivalence: 20000 env combinations, 19690 produced output, 0 mismatches
```

Status: **verified by execution, byte-identical output.**

Two details the equivalence run surfaced and which the proposal keeps honest:

- The head calls `logProviderBackupLFS("Gitlab", envGitLabBackupLFS)` at `backup.go:273` while the
  two lines above it pass `"GitLab"`. This preserves a pre-existing log typo, but it now reads as a
  mistyped argument rather than a deliberate literal, with no comment. Either fix the label (a
  one-character log change) or carry it as an explicit, commented override.
- `logProviderOrgs` centralises the misspelling `Organistations` (`backup.go:205`). That is now a
  one-line fix if log output is allowed to change.

### Worked proposal, stage B (needs two decisions)

Extend the row with `enabled func() bool` and `run func(string) *ProviderBackupResults` so that
`collectProviderBackupResults` is the same loop, BitBucket stops being a special case
(`enabled: bitbucketDefined`), and `checkProvidersDefined` can count enabled rows. Not verified by
execution, and not purely mechanical, for two reasons that need the author's call:

- Run order (`backup.go:100-114`: BitBucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut)
  differs from display order (`backup.go:195-199`: GitHub, Gitea, GitLab, BitBucket, Azure DevOps).
  Run order shapes the `results` array in the webhook payload and the order of errors in
  notifications, so it is the one to hold fixed; a single table then reorders startup log lines.
- Using the run predicate for display would start printing BitBucket configuration for OAuth-only
  deployments. That is the correct behaviour, but it is a visible change.

## F2 — The BitBucket credential predicate, four spellings

### Evidence

`bitbucketAPITokenDefined` and `bitbucketOAuthDefined` (`backup.go:125-138`) hard-code the same
env-var lists that `enabledProviderAuth` already holds at `constants.go:131-139`. The same two
boolean expressions still appear verbatim in `bitbucket.go:24-25`, which the PR did not touch, and
a fourth implementation of "every credential variable is set" is `checkUserAndPasswordProvider`
(`backup.go:369-395`), which the BitBucket entries of `userAndPasswordProviders` would route to —
except that `checkProvidersDefined` intercepts both names in its `switch` (`backup.go:689-697`) so
`checkProvider` is never called for them.

The spellings have already drifted. Scratch probe output:

```
BitBucket email/token = single space -> bitbucketAPITokenDefined=true checkUserAndPasswordProvider=0 checkProvidersDefined err=<nil>
only BITBUCKET_EMAIL set -> checkProvidersDefined err="no providers defined" ; checkProvider(BitBucketAPIToken) count=0 err="BitBucketAPIToken parameter 'BITBUCKET_API_TOKEN' is not defined.\n"
only AZURE_DEVOPS_USERNAME set -> checkProvidersDefined err="AzureDevOps parameter 'AZURE_DEVOPS_PAT' is not defined.\n"
```

Blank-only credentials satisfy the new predicates but not the generic checker, and a half-configured
BitBucket is reported as "no providers defined" while a half-configured Azure DevOps names the
missing parameter. Status: **verified by execution.**

### Worked proposal

```go
// allEnvSet reports whether every variable resolves to a non-empty value.
func allEnvSet(envVars []string) bool {
	for _, envVar := range envVars {
		if val, _ := GetEnvOrFile(envVar); val == "" {
			return false
		}
	}

	return true
}

func bitbucketDefined() bool {
	return allEnvSet(enabledProviderAuth[providerNameBitBucketAPIToken]) ||
		allEnvSet(enabledProviderAuth[providerNameBitBucketOAuth])
}
```

`checkProvidersDefined` adds one to `count` when the API pair is complete, or when the OAuth triple
is complete and the API pair is not; that is exactly `bitbucketDefined()`, so the
`bitbucketAPITokenComplete` local, the "only count if" comment and both `case` arms collapse to a
single `if bitbucketDefined() { count++ }` outside the loop, and `backup.go:101` becomes the same
call.

Verification: scratch probe `TestProbe2Equivalence` compared `allEnvSet(enabledProviderAuth[...])`
with the head's two predicates across all 243 unset / empty / value states of the five BitBucket
variables: `243 states, 0 mismatches`. One non-result difference to be aware of: `allEnvSet`
short-circuits, so when a `*_FILE` indirection points at an unreadable file, fewer
"file … does not exist" lines are logged by `GetEnvOrFile`. Status: **verified by execution.**

A note in the PR's favour: hoisting `bitbucketAPITokenComplete` out of the map loop
(`backup.go:685`) removed a dependency on Go's randomised map iteration order — on the base, with
both methods complete, `count` was 1 or 2 depending on which key was visited first. Only
`count == 0` is ever tested, so nothing observable changed. Read from the diff, not executed.

## F6 — `checkProvider` helpers: out-parameter builder, double pass, stale contract

### Evidence

`checkJustTokenProvider` and `checkUserAndPasswordProvider` (`backup.go:348-395`) take a
`*strings.Builder` out-parameter. Errors are therefore formatted into a builder in the helper,
turned into an `error` in `checkProvider` (`backup.go:339-341`), and immediately flattened back
into a second builder in `checkProvidersDefined` (`backup.go:702-704`). On the base the builder was
a local of one function; the extraction promoted it to shared mutable state.

`checkUserAndPasswordProvider` resolves every credential twice — once to count
(`backup.go:372-379`) and again to name the missing ones (`backup.go:382-387`). `GetEnvOrFile`
opens and reads the `*_FILE` target on each call (`envfile.go:24-58`), so file-backed secrets are
read twice and a missing file is logged twice.

`checkProvider`'s doc comment (`backup.go:322-325`) says it returns "the number of fully-configured
provider entries it found (0 or 1)". The new helper's own comment (`backup.go:346-347`) says it
"counts the provider's non-empty auth parameters", which is what it does, and the probe above shows
`count=2` for Gitea. The two comments now contradict each other four lines apart.

### Worked proposal

```go
func checkUserAndPasswordProvider(provider string) (int, error) {
	params := enabledProviderAuth[provider]

	var missing []string

	for _, param := range params {
		if val, _ := GetEnvOrFile(param); strings.Trim(val, " ") == "" {
			missing = append(missing, param)
		}
	}

	switch len(missing) {
	case 0:
		return 1, nil
	case len(params):
		return 0, nil
	}

	var msg strings.Builder
	for _, param := range missing {
		_, _ = fmt.Fprintf(&msg, "%s parameter '%s' is not defined.\n", provider, param)
	}

	return 0, errors.New(msg.String())
}
```

Verification: scratch probe `TestProbe2Equivalence` ran this against the head's helper for every
unset / empty / blank / value state of every `userAndPasswordProviders` entry:
`96 states, 0 mismatches` on both the returned count and the error text. Status: **verified by
execution.**

With the same `(int, error)` shape on the token-provider helper, `checkProvider` reduces to a
two-way dispatch, and once F1 stage B gives each provider a single `enabled` predicate the
`justTokenProviders` / `userAndPasswordProviders` lists and both `slices.Contains` lookups
(`backup.go:331,335`) can go entirely.
