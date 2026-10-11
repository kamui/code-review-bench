# 01 — Provider configuration: gates, validation, startup display

Covers summary findings F1, F2 and F7, and questions Q1 and Q2. All line numbers are on the head (`136a485`) unless marked "base".

## Measurements

| Measure | Base (`c77f548`) | Head (`136a485`) |
| --- | --- | --- |
| `internal/backup.go` lines | 729 | 774 |
| `internal/backup.go` top-level functions | 23 | 42 |
| `GetEnvOrFile(` call sites in `backup.go` | 45 | 24 |
| `internal/backup_test.go` lines | 1,016 | 989 |
| `internal/notify.go` lines | 248 | 238 |

Commands (run from the clone root):

```
git show main:internal/backup.go | wc -l ; wc -l internal/backup.go
git show main:internal/backup.go | grep -c '^func ' ; grep -c '^func ' internal/backup.go
git show main:internal/backup.go | grep -c 'GetEnvOrFile(' ; grep -c 'GetEnvOrFile(' internal/backup.go
```

The drop in `GetEnvOrFile` call sites is the PR's real win and comes almost entirely from the four `logProvider*` helpers (`backup.go:202-231`). The rise in function count is the cost: nineteen new names, of which five (`display*StartupConfig`) are one-per-provider and four (`logRequestTimeout`, `validateStartupConfig`, `createWorkingDir`, `scheduleBackups`) are sequential slices of `Run`.

## Verification: base-versus-head differential

The clone is read-only, so both trees were exported to the work directory and the same scratch test was dropped into each:

```
git archive main        | tar -x -C <work>/scratch/base
git archive review-head | tar -x -C <work>/scratch/head
cp <work>/scratch/zz_char_test.go <work>/scratch/{base,head}/internal/
ZZ_OUT=<work>/scratch/<v>.out go test ./internal/ -run 'TestZZCharacterize$' -count=1   # in each tree
cmp <work>/scratch/base.out <work>/scratch/head.out
```

with `GOMODCACHE`/`GOCACHE` pointed at the attempt cache and `GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local`.

The harness (`<work>/scratch/zz_char_test.go`) swaps the package `logger` for a buffer and, for 4,000 cases from a fixed-seed generator, assigns each of 32 provider-related variables one of: unset; set to one of `""`, `" "`, `x`, `REFS`, `refs`, `true`, `yes`, `3`, `Org-A,Org-B`; or (one time in twelve) unset with its `_FILE` companion pointing at a non-existent path. For each case it records the output of `displayStartupConfig()`, the logs and error of `checkProvidersDefined()` (line-sorted, because that function ranges over a map on both sides), and the return value, error and logs of `checkProvider(p)` for all seven provider names.

Result: both runs pass, both outputs are 78,083 lines, and `cmp` reports them identical. The head output contains 12,274 organisation / compare-method / LFS display lines and 13,977 "is not defined" validation lines, so the comparison is not vacuous.

Also run on the untouched clone: `go test ./internal/ -count=1` → `ok github.com/jonhadfield/soba/internal 5.249s`; `go vet ./...` → no output. Live-provider tests skip for lack of credentials.

Not covered by the differential: `collectProviderBackupResults` (it calls the provider entry points, which need the network) and `Run`/`scheduleBackups` (scheduler and signal handling). Those were verified by reading only.

### One residual delta, by reading

Base `runProviderBackups` evaluated all five Bitbucket lookups unconditionally (base `backup.go:69-76`). Head `collectProviderBackupResults` evaluates `bitbucketAPITokenDefined() || bitbucketOAuthDefined()` (`:101`), which short-circuits. `GetEnvOrFile` logs when a `_FILE` path cannot be opened (`envfile.go:35-41`). So with complete API-token credentials and, say, `BITBUCKET_USER_FILE` pointing at a missing file, base logged the diagnostic in `runProviderBackups` and again in `Bitbucket()`; head logs it only in `Bitbucket()`. This is harmless and arguably better, but it is a counter-example to "log output unchanged" taken literally. Not executed.

## F1 — the gate is encoded in many places

### Evidence

Where "is provider X configured?" is decided on the head:

| Site | Rule |
| --- | --- |
| `backup.go:105-120` local `tokenProviders` table | one variable non-empty: `GITEA_TOKEN`, `GITHUB_TOKEN`, `GITLAB_TOKEN`, `AZURE_DEVOPS_USERNAME`, `SOURCEHUT_PAT` |
| `backup.go:125-138` `bitbucketAPITokenDefined` / `bitbucketOAuthDefined` | all of a hard-coded list non-empty (no whitespace trim) |
| `backup.go:234, 249, 260, 277, 287` five display guards | one variable non-empty; Bitbucket uses `BITBUCKET_EMAIL` only; no Sourcehut guard at all |
| `constants.go:120-159` `enabledProviderAuth` + `justTokenProviders` + `userAndPasswordProviders`, read by `backup.go:326-395` | any-of (token providers) or all-of (user/password providers), whitespace-trimmed |
| each provider entry function, e.g. `azure_devops.go:13-30`, `bitbucket.go:15-27` | re-checks its own credentials and returns an error result |

Name collision: `justTokenProviders` (`constants.go:149-154`) is `{GitHub, GitLab, Gitea, Sourcehut}`. The new local `tokenProviders` (`backup.go:105`) is `{Gitea, GitHub, GitLab, AzureDevOps, Sourcehut}` and its Azure row is keyed on a username. Azure DevOps is in `userAndPasswordProviders`, not `justTokenProviders`.

Duplicate lists: `enabledProviderAuth[providerNameBitBucketAPIToken]` is `{envBitBucketEmail, envBitBucketAPIToken}` and `[providerNameBitBucketOAuth]` is `{envBitBucketUser, envBitBucketKey, envBitBucketSecret}` (`constants.go:133-141`) — exactly the variables the two new helpers spell out again.

Measured disagreements on the head, from the differential output:

- 128 of 4,000 cases had `BITBUCKET_USER`, `BITBUCKET_KEY` and `BITBUCKET_SECRET` all non-blank and `BITBUCKET_EMAIL` not set to a non-empty value. In those cases the startup block contained 0 lines beginning `BitBucket `.
- In 759 cases `GITEA_APIURL` was non-blank, `GITEA_TOKEN` was not set at all, and `checkProvider("Gitea")` returned 1 — i.e. validation counts the provider as defined while the run table at `:109` would not start it.

Both behaviours are identical on base. They are cited to show why a single definition matters, not as regressions.

Dead half of the predicate: every `false` return of `GetEnvOrFile` carries `""` (`envfile.go:28, 41, 49, 55`), and the `true` returns carry the value (`:18, :21, :58`). Therefore `exists && val != ""` is equivalent to `val != ""`. The predicate lines over `GetEnvOrFile` are `backup.go:42, 117, 129, 137, 191, 204, 211, 234, 249, 260, 265, 277, 287` (thirteen; `:445` is the same shape over `os.LookupEnv`). Of these, `117, 129, 137, 234, 249, 260, 277, 287` use the result purely as a boolean gate — eight lines, eleven lookups.

### Worked proposal, step 1 (behaviour-identical, in scope)

```go
// envSet reports whether envVar, or the file named by envVar_FILE, holds a
// non-empty value.
func envSet(envVar string) bool {
	val, _ := GetEnvOrFile(envVar)

	return val != ""
}

// envAllSet reports whether every variable is set. All are read, so a bad
// _FILE path is still logged for each.
func envAllSet(envVars ...string) bool {
	all := true
	for _, envVar := range envVars {
		all = envSet(envVar) && all
	}

	return all
}

func bitbucketAPITokenDefined() bool {
	return envAllSet(enabledProviderAuth[providerNameBitBucketAPIToken]...)
}

func bitbucketOAuthDefined() bool {
	return envAllSet(enabledProviderAuth[providerNameBitBucketOAuth]...)
}
```

Each display guard becomes `if !envSet(envGitHubToken) { return }`, the table loop becomes `if envSet(p.envVar)`, and the table is renamed (for example `gatedProviders`, with the field called `gateEnv`) so it stops reading as a synonym of `justTokenProviders`.

Verification status: executed. This step was applied to a scratch copy of the head together with the F2, F3 and F4 proposals (see "Proposal check" at the end of this file); the differential output for `displayStartupConfig` and `checkProvidersDefined` is identical to the head's. The rename was not applied, being cosmetic.

### Worked proposal, step 2 (the code-judo move; a separate PR)

```go
type provider struct {
	enabled func() bool
	run     func(backupDir string) *ProviderBackupResults
	startup func() // logs the provider's settings; nil if it has none
}

var providers = []provider{
	{bitbucketDefined, Bitbucket, logBitBucketStartupConfig},
	{func() bool { return envSet(envGiteaToken) }, Gitea, logGiteaStartupConfig},
	{func() bool { return envSet(envGitHubToken) }, GitHub, logGitHubStartupConfig},
	{func() bool { return envSet(envGitLabToken) }, Gitlab, logGitLabStartupConfig},
	{func() bool { return envSet(envAzureDevOpsUserName) }, AzureDevOps, logAzureDevOpsStartupConfig},
	{func() bool { return envSet(envSourcehutToken) }, Sourcehut, nil},
}
```

`collectProviderBackupResults` becomes a four-line loop with no Bitbucket special case; `displayStartupConfig` becomes the same loop calling `startup`; the five display functions lose their guards and shrink to their three or four `logProvider*` calls; the local table is gone.

Why this cannot ride along in a PR that promises unchanged logs:

1. Order. Startup display order today is GitHub, Gitea, GitLab, BitBucket, Azure DevOps (`backup.go:195-199`); run order is BitBucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut (`:101-114`). One slice means one order, so one of the two changes.
2. The Bitbucket display gate changes from "e-mail set" to "either credential set complete" (Q1).
3. Sourcehut would need a decision on whether it prints anything.

Each of those is a small, defensible improvement. They should be made deliberately and stated in the PR body.

## F2 — the count contract

### Evidence

- `backup.go:322-325` (unchanged by the PR): "returns the number of fully-configured provider entries it found (0 or 1)".
- `backup.go:346-347` (new): "counts the provider's non-empty auth parameters".
- Differential, head: `checkProvider(Gitea)` → 0 in 1,470 cases, 1 in 1,905, **2 in 625**; `checkProvider(Sourcehut)` → 0 in 1,522, 1 in 1,891, **2 in 587**. Both providers have two entries in `enabledProviderAuth` (`constants.go:142-149`).
- The sole consumer: `backup.go:712` `if count == 0`. Writers: `:332, :336, :360, :691, :696, :700`.
- The double-count guard at `:694-697` and the hoisted `bitbucketAPITokenComplete` at `:685` exist only to keep `count` at 1 rather than 2 when both Bitbucket methods are complete. (Hoisting it did fix a latent map-order dependence in the base code, where the flag was set inside the `range` and so only suppressed the double count when the API-token key happened to be visited first. Since nothing reads the magnitude, neither version is observable.)
- Double wrapping: helpers write to `outputErrs` (`:327`), `checkProvider` wraps it (`:340`), `checkProvidersDefined` unwraps it into `errBuilder` (`:702-704`) and wraps again (`:709`).
- Unreachable in production: `providerNameBitBucketAPIToken` and `providerNameBitBucketOAuth` are in `userAndPasswordProviders` (`constants.go:155-159`), but the `switch` at `:688-697` handles both names before the `default` that calls `checkProvider`. `grep` finds no other caller of `checkProvider` outside the scratch harness. So `checkUserAndPasswordProvider` runs for Azure DevOps only.
- Divergent rule: `checkUserAndPasswordProvider` trims spaces (`strings.Trim(val, " ") != ""`, `:376`); `bitbucket*Defined` do not (`:129, :137`).

### Worked proposal (error text identical; executed on the scratch copy)

```go
func checkProvidersDefined() error {
	var errs strings.Builder

	// Bitbucket accepts either of two credential sets and reports no
	// partial-configuration errors.
	defined := bitbucketAPITokenDefined()
	defined = bitbucketOAuthDefined() || defined

	for provider := range enabledProviderAuth {
		switch {
		case slices.Contains(justTokenProviders, provider):
			defined = tokenProviderDefined(provider, &errs) || defined
		case provider == providerNameAzureDevOps:
			defined = allAuthParamsDefined(provider, &errs) || defined
		}
	}

	if errs.Len() > 0 {
		return errors.New(errs.String())
	}

	if !defined {
		return errors.New("no providers defined")
	}

	return nil
}
```

`tokenProviderDefined` and `allAuthParamsDefined` are today's two helpers with `int` replaced by `bool`. What disappears: `checkProvider` (19 lines), the `count` variable in four functions, the second `strings.Builder` and the wrap/unwrap/wrap, the double-count guard, and `userAndPasswordProviders` (which would have no remaining reader). The Bitbucket predicates are evaluated before the loop, which is where the head already put one of them.

If the larger change is not wanted, the minimum is to correct the comment at `:322-325` so it agrees with `:346-347` and with what the function measurably returns.

## F7 — no credential-free test for the refactored code

`grep -c '^func Test' internal/backup_test.go` counts 28 tests; the ones that reach `displayStartupConfig`, `checkProvider` or the provider table all call `Run()` behind a credential check and skip here. `TestCheckProvidersFailureWhenNoneDefined` (`backup_test.go:787-802`) is the only credential-free test of the validation path, and it covers the empty environment only.

A reduced form of the scratch harness makes a suitable permanent test:

```go
func TestDisplayStartupConfig(t *testing.T) {
	cases := []struct {
		name string
		env  map[string]string
		want []string
	}{
		{"gitlab defaults", map[string]string{envGitLabToken: "x"},
			[]string{"GitLab project minimum access level: 20", "GitLab compare method: clone"}},
		{"gitlab lfs label", map[string]string{envGitLabToken: "x", envGitLabBackupLFS: "true"},
			[]string{"GitLab project minimum access level: 20", "GitLab compare method: clone", "Gitlab backup LFS: true"}},
		{"bitbucket oauth only prints nothing", map[string]string{envBitBucketUser: "u", envBitBucketKey: "k", envBitBucketSecret: "s"}, nil},
		// ...
	}
	// for each: unsetEnvVarsExcept, t.Setenv, swap logger for a bytes.Buffer, compare lines
}
```

The third case pins today's behaviour for Q1, so that changing it later is a visible decision.

## Minor notes (not findings)

- `logProviderBackupLFS` (`:227-231`) and the skip-user-repos line (`:240`) keep `GetEnvOrFile(envVar); exists && envTrue(envVar)`. `envTrue` reads `os.Getenv` only (`:719-735`), and it can be true only when the variable is set and non-empty, which already implies `exists`. The lookup is redundant apart from its `_FILE` logging side effect.
- `logProviderOrgs` (`:203-207`) keeps `strings.ToLower(orgs) != ""`, which is `orgs != ""`, and lower-cases twice.
- `displayGitLabStartupConfig` passes the label `"GitLab"` twice and `"Gitlab"` once (`:271-273`). This faithfully preserves the base output, but nothing marks the odd spelling as deliberate. The `Organistations` misspelling is now in exactly one format string (`:205`), which makes it a one-character fix whenever log changes are acceptable.

## Proposal check

To confirm the worked proposals are real code and not sketches, the following were applied together to `<work>/scratch/prop` (a copy of the exported head): `envSet` / `envAllSet` with the two Bitbucket predicates derived from `enabledProviderAuth` and the six boolean gates rewritten (F1 step 1); the `bool` contract with a single builder, `checkProvider` deleted and `userAndPasswordProviders` removed from `constants.go` (F2); `createWorkingDir` calling `resolveWorkingDir` (F3); and the flat `scheduleBackups` with `runScheduledJob` deleted (F4).

```
gofmt -l internal/            # no output
go build ./...                # ok
go vet ./internal/            # no output
ZZ_OUT=<work>/scratch/prop.out go test ./internal/ -count=1
                              # ok  github.com/jonhadfield/soba/internal  5.446s
cmp <work>/scratch/head.reduced.out <work>/scratch/prop.out   # identical
```

Because the proposal deletes `checkProvider`, the comparison uses a reduced harness (`<work>/scratch/zz_char_reduced_test.go`) that records `displayStartupConfig` and `checkProvidersDefined` only; it was run on both the head copy and the proposal copy over the same 4,000 cases, giving 39,092 lines each.

| Measure | Head | Head + proposals |
| --- | --- | --- |
| `internal/backup.go` lines | 774 | 730 |
| `internal/backup.go` top-level functions | 42 | 42 (−`checkProvider`, −`runScheduledJob`, +`envSet`, +`envAllSet`) |
| package-level provider lists in `constants.go` | 3 | 2 |

Not covered by this check: the interval and cron arms of `scheduleBackups` (they block until a signal), `createWorkingDir` beyond compilation, and `collectProviderBackupResults` (network). The registry (F1 step 2), the status type (F5) and the table-driven Gitea test (F6) were not applied.
