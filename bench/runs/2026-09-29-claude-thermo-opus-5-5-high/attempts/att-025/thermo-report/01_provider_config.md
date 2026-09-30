# 01 — Provider selection, startup logging and provider validation (`internal/backup.go`)

Scope: `collectProviderBackupResults`, `bitbucketAPITokenDefined`, `bitbucketOAuthDefined`, the five `display*StartupConfig` functions and their four `logProvider*` helpers, `checkProvider` / `checkJustTokenProvider` / `checkUserAndPasswordProvider`, and `checkProvidersDefined`.

Commands used:

```
git diff main...review-head -- internal/backup.go
grep -n "^func " internal/backup.go
sed -n 100,160p internal/constants.go          # enabledProviderAuth, justTokenProviders, userAndPasswordProviders
grep -n "checkProvidersDefined\|checkProvider(" internal/*_test.go
go vet ./internal/ && go test ./internal/ -count=1   # ok (live-provider tests skip without credentials)
```

Measurements: `internal/backup.go` goes from 729 to 774 lines (no 1k threshold concern). The number of top-level functions touched by this area rises from 4 (`runProviderBackups`, `displayStartupConfig`, `checkProvider`, `checkProvidersDefined`) to 17.

---

## Finding P1 — The refactor adds a sixth provider registry instead of collapsing the five that already exist (missed code-judo move)

**Where:** `internal/backup.go:97-137` (`collectProviderBackupResults` and its inline `tokenProviders` table), `internal/backup.go:190-294` (`displayStartupConfig` and five `display*StartupConfig` functions), `internal/constants.go:120-159` (`enabledProviderAuth`, `justTokenProviders`, `userAndPasswordProviders`).

**Evidence.** After this PR, knowledge about "what is a provider and how is it configured" lives in these independent places, each keyed differently:

1. `enabledProviderAuth` (constants.go:120) — provider name → required auth env vars.
2. `justTokenProviders` / `userAndPasswordProviders` (constants.go:149-159) — validation strategy by provider name.
3. `checkProvidersDefined` (backup.go:680) — a hard-coded `switch` that special-cases the two Bitbucket names.
4. New in this PR: the anonymous `tokenProviders` struct slice (backup.go:105-115) — gate env var → backup function, plus a separate Bitbucket `if` before it.
5. New shape in this PR: five hand-written `display*StartupConfig` functions (backup.go:233-294), each with its own gate check and its own subset of `logProvider*` calls with per-provider labels.

The gates disagree with one another, and the refactor hid that instead of exposing it:

- Running Bitbucket needs `bitbucketAPITokenDefined() || bitbucketOAuthDefined()` (backup.go:101), but `displayBitBucketStartupConfig` is gated on `envBitBucketEmail` alone (backup.go:277), so an OAuth-only Bitbucket user gets no startup config logged.
- Azure DevOps runs when only the username is set (the `tokenProviders` row keys on `envAzureDevOpsUserName`), but validation (`checkUserAndPasswordProvider` via `enabledProviderAuth`) requires username **and** PAT.
- Sourcehut is backed up and validated but has no startup-config function at all.
- Gitea runs on the token alone, but `enabledProviderAuth` lists the API URL too.

These inconsistencies were already there before the PR. What matters here is that the PR was a structural refactor of every one of these sites, and it rearranged them into more functions without reducing the number of concepts. The Sonar complexity score dropped because the logic is now spread across more places. The design did not get simpler.

**Code-judo proposal (behaviour-preserving first step, then optional consistency fixes).** Introduce one typed descriptor table and derive all three flows from it:

```go
type providerSpec struct {
    name       string                                   // providerNameGitHub, ...
    label      string                                   // "GitHub", used in log lines
    enabled    func() bool                              // single gate used by run AND display
    run        func(backupDir string) *ProviderBackupResults
    orgsEnv    string                                   // "" when not applicable
    backupsEnv string
    compareEnv string
    lfsEnv     string
    extraLog   func()                                   // GitHub skip-user-repos, GitLab min access level
}

var providers = []providerSpec{
    {name: providerNameGitHub, label: "GitHub", enabled: envSet(envGitHubToken), run: GitHub,
     orgsEnv: envGitHubOrgs, compareEnv: envGitHubCompare, lfsEnv: envGitHubBackupLFS, extraLog: logGitHubSkipUserRepos},
    {name: providerNameGitea, label: "Gitea", enabled: envSet(envGiteaToken), run: Gitea,
     orgsEnv: envGiteaOrgs, backupsEnv: envGiteaBackups, compareEnv: envGiteaCompare, lfsEnv: envGiteaBackupLFS},
    // GitLab, Bitbucket (enabled: bitbucketEnabled), Azure DevOps, Sourcehut ...
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

func displayProviderStartupConfig() {
    for _, p := range providers {
        if !p.enabled() { continue }
        if p.orgsEnv != ""    { logProviderOrgs(p.label, p.orgsEnv) }
        if p.backupsEnv != "" { logProviderBackupsToKeep(p.label, p.backupsEnv) }
        if p.extraLog != nil  { p.extraLog() }
        if p.compareEnv != "" { logProviderCompareMethod(p.label, p.compareEnv) }
        if p.lfsEnv != ""     { logProviderBackupLFS(p.label, p.lfsEnv) }
    }
}
```

This removes `displayGitHubStartupConfig`, `displayGiteaStartupConfig`, `displayGitLabStartupConfig`, `displayBitBucketStartupConfig` and `displayAzureDevOpsStartupConfig`, the separate Bitbucket branch in `collectProviderBackupResults`, and the anonymous `tokenProviders` struct. Adding a provider then means adding one row instead of editing five places. To keep log output byte-identical in the first step, each row can carry its current gate exactly (for example, Bitbucket display keeps its email-only gate through a `displayEnabled` override), and the ordering of log calls can be kept per row. In a second, deliberate step, the author can unify the gates. That changes behaviour, but only the startup log lines, and it removes the inconsistencies listed above. `enabledProviderAuth` and the two strategy slices can then become fields on the same spec (`authEnvs []string`, `auth authKind`), which removes the name-keyed map lookups in `checkProvider`.

**Verification status:** I confirmed the gate mismatches by reading the code (backup.go:101, 105-115, 234, 249, 260, 277, 287; constants.go:120-159). The proposal is not implemented or tested. The existing test suite passes on the head (`go test ./internal/ -count=1` → ok).

---

## Finding P2 — `checkProvidersDefined` keeps dead Bitbucket de-dup arithmetic, and the Bitbucket entries in `userAndPasswordProviders` are unreachable

**Where:** `internal/backup.go:680-717`, `internal/backup.go:369-395`, `internal/constants.go:155-159`.

**Evidence.** `count` in `checkProvidersDefined` is only ever read as `count == 0` (backup.go:712). The "Only count if the API OAuthToken method isn't already complete" rule (backup.go:693-697) therefore cannot change the result, whether or not the API-token path is complete. The loop also intercepts `providerNameBitBucketAPIToken` and `providerNameBitBucketOAuth` before the `default:` branch, so `checkProvider` never sees them. Their membership in `userAndPasswordProviders` is dead data, and the partial-config diagnostics that `checkUserAndPasswordProvider` would produce ("BitBucket parameter 'X' is not defined") never fire for Bitbucket. The PR extracted `checkUserAndPasswordProvider` as a first-class helper without noticing that two of its three nominal callers can't reach it.

There is also a small, harmless semantic shift. Before the PR, `bitbucketAPITokenComplete` was only set when the map iteration reached the API-token case, so with both Bitbucket methods configured, `count` was 1 or 2 depending on map order. The PR hoists the check, which makes it deterministic. No behaviour can be observed because only `count == 0` is tested.

**Remedy.** Make the contract honest: replace `count int` with `anyDefined bool` and delete the de-dup comment and branch:

```go
anyDefined := bitbucketAPITokenDefined() || bitbucketOAuthDefined()
for provider := range enabledProviderAuth {
    if provider == providerNameBitBucketAPIToken || provider == providerNameBitBucketOAuth {
        continue
    }
    ok, err := checkProvider(provider)
    anyDefined = anyDefined || ok
    ...
}
```

Better still, with the P1 descriptor table, `anyDefined` is just "some `providerSpec.enabled()` is true". That is the same predicate the run path uses, so validation and execution can no longer disagree. Then either drop the Bitbucket names from `userAndPasswordProviders`, or deliberately route them through `checkProvider` so partial Bitbucket configs get the same diagnostics as Azure DevOps (a behaviour change for the author to decide; see question Q1 in the summary).

**Verification status:** Confirmed by reading. `TestCheckProvidersFailureWhenNoneDefined` (backup_test.go:786) is the only direct test of `checkProvidersDefined` and covers only the zero case.

---

## Finding P3 — `checkUserAndPasswordProvider` does two passes over the same env vars and returns a bool disguised as `int`

**Where:** `internal/backup.go:326-395`.

**Evidence.** `checkUserAndPasswordProvider` counts `foundCount`/`totalCount` in one loop, then loops again calling `GetEnvOrFile` and `strings.Trim` a second time to find the missing ones, and returns the literal `1` or `0`. `checkJustTokenProvider` returns a count of non-empty parameters, which `checkProvider` sums with the other result. The only consumer (P2) treats the sum as a boolean. `checkProvider` also runs two independent `if slices.Contains(...)` checks, although each provider belongs to exactly one strategy list.

**Remedy.** Do a single pass that collects `missing []string`, then decide:

```go
func checkUserAndPasswordProvider(provider string, errs *strings.Builder) bool {
    var missing []string
    for _, param := range enabledProviderAuth[provider] {
        if v, ok := GetEnvOrFile(param); !ok || strings.TrimSpace(v) == "" {
            missing = append(missing, param)
        }
    }
    if len(missing) > 0 && len(missing) < len(enabledProviderAuth[provider]) {
        for _, p := range missing { fmt.Fprintf(errs, "%s parameter '%s' is not defined.\n", provider, p) }
    }
    return len(missing) == 0
}
```

(Keep `strings.Trim(v, " ")` if the exact trimming semantics must be preserved. `TrimSpace` also strips tabs and newlines.) Turn `checkProvider` into a `switch` on the strategy, or better, a field on the P1 spec. Both helpers return `bool`.

**Verification status:** Confirmed by reading. Not implemented.

---

## Finding P4 — Centralised log helpers now own a typo and a label inconsistency

**Where:** `internal/backup.go:203-207` (`logProviderOrgs`), `internal/backup.go:273`.

**Evidence.** `logProviderOrgs` hard-codes `"%s Organistations: %s"`, and `displayGitLabStartupConfig` passes `"Gitlab"` to `logProviderBackupLFS` while passing `"GitLab"` to its other helpers on the lines just above. Both spellings reproduce the old output exactly, which matches the PR's "log output unchanged" promise. Now that there is one helper and one label per provider, fixing them is a one-line change, and leaving them makes the new shared helper the canonical source of the typo. With the P1 table, the label exists once per provider, so the second inconsistency cannot happen.

**Remedy.** Fix the spelling in `logProviderOrgs` and use `"GitLab"` consistently, either in this PR or as an explicit follow-up. If anyone greps logs for the old strings, note the change in the release notes.

**Verification status:** Confirmed by reading and by `git diff` comparison with the pre-PR strings.

---

## Behaviour-preservation notes (verified, no finding)

- The startup log strings are byte-identical for every provider. `logger.Print("X compare method: refs")` became `logger.Printf("%s compare method: %s", "X", compareTypeRefs)`, with `compareTypeRefs = "refs"` and `compareTypeClone = "clone"` (constants.go:108-109). The Bitbucket comparison changed from `strings.ToLower(c) == "refs"` to `strings.EqualFold(c, "refs")`. The two are equivalent for any realistic input. The only divergence is Unicode case-folding (for example U+017F "ſ" folds to "s"), which is not a practical concern.
- `collectProviderBackupResults` keeps the original provider order (Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut).
