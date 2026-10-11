# 01 — Provider gating, validation and startup display (`internal/backup.go`)

Covers summary findings F2, F3, F4, F5 and F6. All line numbers are at the head, `136a485`. The harness and the scratch copies referred to here are described in `03_notify_tests_dockerfile.md`, section 5.

## Measurements for this area

| | base `c77f548` | head `136a485` | scratch proposal |
| --- | --- | --- | --- |
| `internal/backup.go` lines | 729 | 774 | 750 |
| `internal/backup.go` top-level functions | 23 | 42 | 44 |
| `GetEnvOrFile` call sites in `backup.go` | 45 | 24 | 11 |

Produced with `wc -l`, `grep -c '^func '` and `grep -c GetEnvOrFile` on `git show main:internal/backup.go`, on the checked-out file, and on `scratch/proposal/internal/backup.go`.

The proposal has two more functions than the head. That is deliberate and I would rather report it than hide it: it adds `envOrFileDefined`, `undefinedParamsError` and `backupSchedule` and removes `runScheduledJob`. Each of the three additions is called from several places or isolates a decision; none is a pass-through.

## 1. `collectProviderBackupResults` and the provider registries (F5)

**Evidence.** `internal/backup.go:97-123`:

```go
// BitBucket - check for API OAuthToken or OAuth2 authentication
if bitbucketAPITokenDefined() || bitbucketOAuthDefined() {
	results = append(results, *Bitbucket(backupDir))
}

tokenProviders := []struct {
	envVar string
	run    func(string) *ProviderBackupResults
}{
	{envGiteaToken, Gitea},
	{envGitHubToken, GitHub},
	{envGitLabToken, Gitlab},
	{envAzureDevOpsUserName, AzureDevOps},
	{envSourcehutToken, Sourcehut},
}
```

The package-level registries it sits beside are in `internal/constants.go:120-159`: the map `enabledProviderAuth` (seven keys, BitBucket split into two), and the slices `justTokenProviders` (GitHub, GitLab, Gitea, Sourcehut) and `userAndPasswordProviders` (both BitBucket methods and Azure DevOps).

**Why this is a problem.** Three things.

The name is wrong. `tokenProviders` contains Azure DevOps, which the package itself classifies under `userAndPasswordProviders`, and its gate column for that row is a username. Anyone who greps for "token providers" finds two lists with different members.

The row shape forces the special case. Because a row is `(envVar, run)`, BitBucket's two-method gate cannot be a row, so the table handles five providers and a hand-written `if` handles the sixth. The table saved Sonar a few decision points without making "run each enabled provider" one uniform loop.

The gate exists twice. Every provider function re-evaluates the same credential and returns an error result if it is missing: `internal/gitea.go:13-24`, `internal/azure_devops.go:13-24`, `internal/bitbucket.go:24-39`, and the same shape in the others. The outer gate decides "skip silently"; the inner gate decides "report failure". Because the outer gate always runs first, the inner branches for the gate variable are unreachable from `runProviderBackups`.

**Remedy, minimum.** Give the table a truthful name and a predicate column so BitBucket is a row:

```go
providers := []struct {
	enabled func() bool
	run     func(backupDir string) *ProviderBackupResults
}{
	{func() bool { return bitbucketAPITokenDefined() || bitbucketOAuthDefined() }, Bitbucket},
	{func() bool { return envOrFileDefined(envGiteaToken) }, Gitea},
	// ...
}
```

This is not obviously shorter than the head and I did not build it. It is listed so the direction is concrete.

**Remedy, ambitious.** One package-level table next to `enabledProviderAuth`, carrying the provider name, its auth parameters, whether it is all-or-nothing, its `enabled` predicate and its run function. `justTokenProviders` and `userAndPasswordProviders` become a field. `collectProviderBackupResults`, `checkProvidersDefined` and `displayStartupConfig` each become a loop over it.

**Why I am not claiming this is a drop-in.** I checked the three orderings and gates before proposing it, and they do not agree:

- Run order is BitBucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut (`backup.go:101-114`).
- Display order is GitHub, Gitea, GitLab, BitBucket, Azure DevOps, with no Sourcehut (`backup.go:195-199`).
- The run gate for BitBucket is "API-token pair or OAuth triple"; the display gate is `BITBUCKET_EMAIL` alone (`backup.go:277`).
- The validation gate trims spaces (`strings.Trim(val, " ")`, `backup.go:357`, `376`) and the run and display gates do not.

A single table would have to carry two orders or change one, and would have to pick one BitBucket gate. Those are behaviour decisions, raised as Q2 in the summary.

**Verification status.** By reading only.

## 2. `checkProvidersDefined` and the dead de-duplication flag (F2)

**Evidence.** `internal/backup.go:680-717`. The relevant lines:

```go
bitbucketAPITokenComplete := bitbucketAPITokenDefined()

for provider := range enabledProviderAuth {
	switch provider {
	case providerNameBitBucketAPIToken:
		if bitbucketAPITokenComplete {
			count++
		}
	case providerNameBitBucketOAuth:
		// Only count if the API OAuthToken method isn't already complete.
		if bitbucketOAuthDefined() && !bitbucketAPITokenComplete {
			count++
		}
	default:
		// ...
	}
}
// ...
if count == 0 {
	return errors.New("no providers defined")
}
```

`count` has exactly one reader, the comparison with zero at line 712. The function returns only an `error`. So "count BitBucket once, not twice" has no observable consequence.

The base makes this plain. There, `bitbucketAPITokenComplete` started `false` and was set to `true` inside the `providerNameBitBucketAPIToken` arm of a `range` over a map. When Go visited the OAuth key first, both arms incremented. The rule was order-dependent for as long as it existed and nothing depended on it.

**Why this is a problem.** The PR needed to reduce the complexity of this function and did so by hoisting the flag, which turns an accidental no-op into a deliberate-looking invariant with a rewritten comment. A reader now has to work out why BitBucket must not be double-counted, and the answer is that it does not matter.

**Worked proposal** (applied in `scratch/proposal`):

```go
func checkProvidersDefined() error {
	var errBuilder strings.Builder

	// BitBucket is all-or-nothing per auth method but, unlike the other
	// providers, a half-configured method is not an error.
	defined := bitbucketAPITokenDefined() || bitbucketOAuthDefined()

	for provider := range enabledProviderAuth {
		if provider == providerNameBitBucketAPIToken || provider == providerNameBitBucketOAuth {
			continue
		}

		n, err := checkProvider(provider)
		defined = defined || n > 0

		if err != nil {
			errBuilder.WriteString(err.Error())
		}
	}

	if errBuilder.Len() > 0 {
		return errors.New(errBuilder.String())
	}

	if !defined {
		return errors.New("no providers defined")
	}

	return nil
}
```

The seed expression is the same one that gates the BitBucket run at line 101, so "BitBucket is configured" is now visibly one rule used in both places. The comment states the one thing that actually is special about BitBucket here, which the head does not say anywhere: a partial BitBucket configuration is tolerated, where a partial Azure DevOps configuration is an error.

One small cost to state: `||` short-circuits, so when the API-token pair is complete the OAuth variables are not read during validation. That only matters if an OAuth `_FILE` variable points at a missing path, in which case one "file … does not exist" log line is no longer printed at this point. `Bitbucket()` reads all five again and prints it then.

**Verification status.** Verified. The differential harness prints the validation verdict (with error lines sorted, because map order varies) for 20,000 environments; base, head and proposal are byte-identical.

## 3. `checkProvider` and its helpers (F3)

**Evidence.** `internal/backup.go:326-395`. `checkProvider` owns a `strings.Builder`, and both helpers take `outputErrs *strings.Builder` and also return an `int`. `checkUserAndPasswordProvider`:

```go
var foundCount, totalCount int

for _, param := range enabledProviderAuth[provider] {
	totalCount++

	val, exists := GetEnvOrFile(param)
	if exists && strings.Trim(val, " ") != "" {
		foundCount++
	}
}

if foundCount > 0 && foundCount < totalCount {
	for _, param := range enabledProviderAuth[provider] {
		val, exists := GetEnvOrFile(param)
		if !exists || strings.Trim(val, " ") == "" {
			_, _ = fmt.Fprintf(outputErrs, "%s parameter '%s' is not defined.\n", provider, param)
		}
	}
}

if foundCount == totalCount {
	return 1
}

return 0
```

**Why this is a problem.** Two return channels per helper. Two passes over the same list with every variable read twice, and `GetEnvOrFile` opens and reads a file when the `_FILE` form is used. `totalCount` is `len(enabledProviderAuth[provider])` computed by hand. The format string for the error line appears in both helpers. And `checkProvider` tests membership of two lists with independent `if`s, which invites the reader to ask what happens for a provider in both or neither; the answer from `constants.go` is that every key is in exactly one, and that the two BitBucket members of `userAndPasswordProviders` never reach this function at all.

**Worked proposal** (applied in `scratch/proposal`):

```go
func checkProvider(provider string) (int, error) {
	if slices.Contains(userAndPasswordProviders, provider) {
		return checkUserAndPasswordProvider(provider)
	}

	return checkJustTokenProvider(provider)
}

// checkJustTokenProvider counts the provider's non-empty auth parameters and
// reports any that are defined but blank.
func checkJustTokenProvider(provider string) (int, error) {
	var count int

	var blank []string

	for _, param := range enabledProviderAuth[provider] {
		val, exists := GetEnvOrFile(param)

		switch {
		case strings.Trim(val, " ") != "":
			count++
		case exists:
			blank = append(blank, param)
		}
	}

	return count, undefinedParamsError(provider, blank)
}

// checkUserAndPasswordProvider returns 1 if all of the provider's auth
// parameters are set, and reports the missing ones when only some are.
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
	default:
		return 0, undefinedParamsError(provider, missing)
	}
}

func undefinedParamsError(provider string, params []string) error {
	if len(params) == 0 {
		return nil
	}

	var b strings.Builder

	for _, param := range params {
		_, _ = fmt.Fprintf(&b, "%s parameter '%s' is not defined.\n", provider, param)
	}

	return errors.New(b.String())
}
```

The three outcomes of the all-or-nothing check (all set, none set, some set) are now three `case` arms instead of two overlapping `if`s on a pair of counters. An empty parameter list still returns 1, as the head does, because `case 0` is tested first.

A stricter reading of the dispatch: the head runs both helpers for a provider that is in both lists and neither for one in none. With the current data that cannot happen, and the proposal's either/or is equivalent. If the author wants to keep the two-`if` form for safety, the helpers can still return `(int, error)`.

This also exposes a quirk that both versions share and that I am leaving alone: for a just-token provider with two parameters, such as Gitea with `GITEA_APIURL` and `GITEA_TOKEN`, setting only the API URL makes the provider count as defined. It is behaviour, not structure, and belongs with the provider-table decision.

**Verification status.** Verified by the differential harness: identical verdicts and identical error lines on 20,000 environments.

## 4. Redundant predicates in the new log helpers and gates (F4)

**Evidence.** `internal/backup.go:202-231`:

```go
func logProviderOrgs(label, envVar string) {
	if orgs, exists := GetEnvOrFile(envVar); exists && strings.ToLower(orgs) != "" {
		logger.Printf("%s Organistations: %s", label, strings.ToLower(orgs))
	}
}
// ...
func logProviderBackupLFS(label, envVar string) {
	if _, exists := GetEnvOrFile(envVar); exists && envTrue(envVar) {
		logger.Printf("%s backup LFS: true", label)
	}
}
```

`GetEnvOrFile` (`internal/envfile.go:14-59`) has four `return "", false` exits and no exit that returns a non-empty string with `false`. So for any caller, a non-empty value implies `exists`. `envTrue` (`backup.go:719-735`) reads `os.Getenv` only, so `envTrue(x)` being true implies the plain variable is set and non-empty, which implies `GetEnvOrFile(x)` returns `true`.

The same `value, exists := GetEnvOrFile(…); exists && value != ""` pattern, or its negation, also appears at lines 41-42, 117, 126-129, 133-137, 191, 211, 234, 240, 249, 260, 277 and 287.

**Why this is a problem.** Extracting a helper is the one moment when a repeated condition gets a single definition. The PR used that moment to copy the condition unchanged, so the redundancy is now a shared, documented helper. The LFS case is worse than redundant, because the `GetEnvOrFile` call implies file-based configuration works for the LFS flags and it does not.

**Worked proposal** (applied in `scratch/proposal`):

```go
// envOrFileDefined reports whether envVar (or its _FILE form) holds a value.
func envOrFileDefined(envVar string) bool {
	val, _ := GetEnvOrFile(envVar)

	return val != ""
}

func bitbucketAPITokenDefined() bool {
	return envOrFileDefined(envBitBucketEmail) && envOrFileDefined(envBitBucketAPIToken)
}

func bitbucketOAuthDefined() bool {
	return envOrFileDefined(envBitBucketUser) && envOrFileDefined(envBitBucketKey) && envOrFileDefined(envBitBucketSecret)
}

func logProviderOrgs(label, envVar string) {
	if orgs, _ := GetEnvOrFile(envVar); orgs != "" {
		logger.Printf("%s Organistations: %s", label, strings.ToLower(orgs))
	}
}

func logProviderBackupLFS(label, envVar string) {
	if envTrue(envVar) {
		logger.Printf("%s backup LFS: true", label)
	}
}
```

and each display function opens with `if !envOrFileDefined(envGitHubToken) { return }` instead of binding a token variable it only compares with the empty string.

`envOrFileDefined` belongs in `envfile.go` beside `GetEnvOrFile`; the provider files (`gitea.go`, `github.go`, and the rest) repeat the same `!exists || x == ""` test and could use it too. I left it in `backup.go` in the scratch copy only to keep the patch to one file.

**Caveat on equivalence.** Removing the `GetEnvOrFile` call from `logProviderBackupLFS` and the skip-user-repos check removes one side effect: if `X_FILE` is set to a path that cannot be opened and `X` itself is unset, the head logs "file … does not exist" from that call and the proposal does not. The short-circuit in the BitBucket predicates has the same character. The harness does not set `_FILE` variables, so this difference is established by reading, not by the run.

**Verification status.** Verified by the differential harness for all environments without `_FILE` variables; the `_FILE` caveat above is by reading.

## 5. Provider labels as repeated literals (F6)

**Evidence.** `grep -n '"GitHub"\|"Gitea"\|"GitLab"\|"Gitlab"\|"BitBucket"\|"Azure DevOps"' internal/backup.go` returns sixteen call sites between lines 238 and 293. `internal/constants.go:98-105` defines `providerNameGitHub = "GitHub"`, `providerNameGitea = "Gitea"`, `providerNameGitLab = "GitLab"`, `providerNameBitBucket = "BitBucket"`. `providerNameAzureDevOps` is `"AzureDevOps"` with no space, so it cannot serve as the display label.

Line 273 is `logProviderBackupLFS("Gitlab", envGitLabBackupLFS)` while lines 271 and 272 pass `"GitLab"`. Line 205 has `Organistations`. Both reproduce the base's output exactly.

**Why this is a problem.** The canonical names were in scope and were not used. And the two historical typos have changed character: one is now an argument that differs from its neighbours, the other is in a helper shared by three providers.

**Remedy.** Pass the four existing constants. For Azure DevOps, add a display-label constant or accept the three literals. For the two typos, either fix them (a log-output change, so a separate commit) or add a one-line comment that they are preserved on purpose. In the scratch copy I switched to the four constants and left both typos as they are.

**A structural alternative I considered and am not pushing.** The five `display…StartupConfig` functions are nearly a table: every provider prints some subset of orgs, backups-to-keep, compare method and LFS, in that order. A struct with those four env-var fields and an optional `extra func()` slot would replace all five functions with one loop. I checked whether it preserves line order, and it does only if the `extra` slot sits between orgs and backups-to-keep (GitHub's "skipping user repos" comes after orgs; GitLab's minimum access level comes first and GitLab has no orgs line). It would also need a second label field solely to carry the `Gitlab` typo. With two of five rows needing an escape hatch and one needing a typo field, the table is not clearly simpler than five five-line functions, so I am leaving this as an observation. It becomes worthwhile if the provider table in section 1 is adopted.

**An equivalence note.** The BitBucket compare check changed from `strings.ToLower(compare) == compareTypeRefs` to `strings.EqualFold(compare, compareTypeRefs)` by going through the shared helper. These differ only for non-ASCII case folds: `"refſ"` (long s) gives `false` for the first and `true` for the second, confirmed with a four-line program in `scratch/fold`. It has no practical consequence and is recorded only because the PR says log output is unchanged.

**Verification status.** The switch to constants is verified by the differential harness. The typos are confirmed by reading and by `grep -rn Organistations`.
