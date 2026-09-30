# 01 — Provider configuration, selection and startup logging (`internal/backup.go`)

Scope: `collectProviderBackupResults`, `bitbucketAPITokenDefined`, `bitbucketOAuthDefined`, `displayStartupConfig` and its five `display*StartupConfig` children plus four `logProvider*` helpers, `checkProvider` / `checkJustTokenProvider` / `checkUserAndPasswordProvider`, and `checkProvidersDefined`.

## Measurements

- `git show main:internal/backup.go | wc -l` → 729; `wc -l internal/backup.go` at head → 774. The file grows by 45 lines and stays well under 1k, so the file-size rule does not apply.
- `grep -n "^func " internal/backup.go` at head: this area alone adds 14 new top-level functions (lines 97–391 and 680). The subsystem is now spread across 20 functions.
- `go vet ./internal/` → clean. `go test ./internal/ -count=1` → `ok github.com/jonhadfield/soba/internal 5.202s`. The live provider tests skip because there are no credentials. None of the new helpers have a direct unit test.
- `git status --short` → empty after the run, so the clone was not modified.

## P1-A — Provider knowledge now lives in four parallel tables and five near-identical functions (missed code-judo)

**Status: verified by reading the head source.**

Before the PR, the set of providers and their environment variables were already spread across `enabledProviderAuth`, `justTokenProviders` and `userAndPasswordProviders` in `internal/constants.go:120-159`, and hand-written `if` blocks in `runProviderBackups` and `displayStartupConfig`. The PR resolves the Sonar complexity findings by adding a fourth provider table and splitting the fifth location into more pieces. The fourth table is the anonymous `tokenProviders` struct slice in `collectProviderBackupResults` (`internal/backup.go:105-114`). The split produces `displayGitHubStartupConfig`, `displayGiteaStartupConfig`, `displayGitLabStartupConfig`, `displayBitBucketStartupConfig` and `displayAzureDevOpsStartupConfig` (`internal/backup.go:233-294`). Each one repeats the same pattern: a `GetEnvOrFile(token); !exists || == ""` guard, then calls to `logProviderOrgs` / `logProviderBackupsToKeep` / `logProviderCompareMethod` / `logProviderBackupLFS`, each given a string label and the matching `env*` constant by hand.

The per-provider facts are the same everywhere: display label, gating credential, orgs variable, backups variable, compare variable, LFS variable, and the backup entry point. They are now written out in `constants.go`, in `tokenProviders`, and in five display functions. Adding a provider, or renaming a provider's label, means editing all of them. The label drift has already started. The LFS line is logged with label `"Gitlab"` (`internal/backup.go:273`), while every other GitLab line uses `"GitLab"` (lines 267-272). The PR kept this on purpose to preserve log output, but it now shows up as a mismatched literal in adjacent calls. Sourcehut is in `tokenProviders` but has no startup-config display. This is a pre-existing gap that a single table would make obvious.

This refactor moves the complexity around without removing any of it. The cognitive-complexity metric went down because each branch became its own function. But a reader still has to keep five hand-maintained provider blocks and a separate selection table in their head.

### Worked code-judo proposal

Add one typed descriptor and let every consumer loop over it:

```go
type providerSpec struct {
    label       string                                // "GitHub", "Azure DevOps", ...
    enabled     func() bool                           // credential gate
    run         func(string) *ProviderBackupResults   // backup entry point
    orgsEnv     string                                // "" when unsupported
    backupsEnv  string
    compareEnv  string
    lfsEnv      string
    extraLog    func()                                // e.g. GitLab min access level, GitHub skip-user-repos
}

var providers = []providerSpec{
    {label: "BitBucket", enabled: bitbucketDefined, run: Bitbucket,
        backupsEnv: envBitBucketBackups, compareEnv: envBitBucketCompare, lfsEnv: envBitBucketBackupLFS},
    {label: "Gitea", enabled: envSet(envGiteaToken), run: Gitea,
        orgsEnv: envGiteaOrgs, backupsEnv: envGiteaBackups, compareEnv: envGiteaCompare, lfsEnv: envGiteaBackupLFS},
    // GitHub, GitLab, Azure DevOps, Sourcehut ...
}

func collectProviderBackupResults(backupDir string) (results []ProviderBackupResults) {
    for _, p := range providers {
        if p.enabled() {
            results = append(results, *p.run(backupDir))
        }
    }
    return results
}

func displayStartupConfig() {
    // root dir line ...
    for _, p := range providers {
        if !p.enabled() { continue }
        logProviderOrgs(p.label, p.orgsEnv)          // helpers no-op on ""
        logProviderBackupsToKeep(p.label, p.backupsEnv)
        if p.extraLog != nil { p.extraLog() }
        logProviderCompareMethod(p.label, p.compareEnv)
        logProviderBackupLFS(p.label, p.lfsEnv)
    }
}
```

This deletes all five `display*StartupConfig` functions and the anonymous `tokenProviders` table, and puts each provider's label in one place. Keeping log output byte-for-byte identical needs two small concessions: the per-provider line order (GitHub logs skip-user-repos between orgs and compare), and the `"Gitlab"` LFS label, which should simply be corrected. The per-provider order of the startup log lines is not a contract worth preserving. If the author wants zero log changes in this PR, the table can still carry an ordered list of log steps. That is more machinery than it is worth, and I would take the small log change instead. Bitbucket ordering changes too: backups currently run Bitbucket first and display logs it fourth. A single table forces one order, which is an improvement.

## P1-B — Bitbucket startup display gates on a different predicate than the one the PR just extracted

**Status: verified by reading the head source. Behaviour is unchanged from base, so this is a missed reuse, not a regression.**

The PR introduces `bitbucketAPITokenDefined()` and `bitbucketOAuthDefined()` (`internal/backup.go:125-138`) as the canonical "is Bitbucket configured" predicates. It uses them in `collectProviderBackupResults` (line 101) and in `checkProvidersDefined` (lines 685-697). `displayBitBucketStartupConfig` (`internal/backup.go:276-284`) still gates on its own ad-hoc test: `GetEnvOrFile(envBitBucketEmail)` being non-empty. So a user who configures Bitbucket through OAuth (`BITBUCKET_USER`/`KEY`/`SECRET`) gets a Bitbucket backup but no Bitbucket startup-config lines. A user who sets only `BITBUCKET_EMAIL` gets Bitbucket startup lines and no Bitbucket backup. After this PR, three sites decide whether Bitbucket is on, and one of them disagrees with the other two.

Remedy: add `bitbucketDefined() bool { return bitbucketAPITokenDefined() || bitbucketOAuthDefined() }` and use it in all three places. In the P1-A table this becomes Bitbucket's `enabled` field, and the inconsistency cannot be expressed at all.

## P2-A — `checkProvidersDefined` keeps a for-switch over map keys; `checkProvider` carries dead Bitbucket paths

**Status: verified by reading the head source. I traced by hand that the change to `bitbucketAPITokenComplete` does not change behaviour.**

`checkProvidersDefined` (`internal/backup.go:680-717`) ranges over the `enabledProviderAuth` map and then `switch`es on the key to special-case the two Bitbucket entries. This is the "loop-switch" anti-pattern. The loop does nothing for those two keys except wait until the map's random iteration order happens to reach them.

The PR changes this function in one semantic way. `bitbucketAPITokenComplete` is now computed before the loop instead of being set when the `BitBucketAPIToken` key is visited. At base, if both Bitbucket methods were configured and the map visited `BitBucketOAuth` first, `count` was incremented twice. That was nondeterministic. Because `count` is only compared with `0` (line 712), the change is unobservable. It is a quiet improvement, and it makes the dedup logic pointless. Nothing outside the function cares whether Bitbucket counts once or twice, so the "Only count if the API OAuthToken method isn't already complete" comment and branch describe an invariant that does not exist.

`userAndPasswordProviders` (`internal/constants.go:155-159`) lists both Bitbucket providers. But `checkProvider` is only called from the `default` arm of this switch (line 699), so `checkUserAndPasswordProvider` is in practice only ever reached for Azure DevOps. The new helper's doc comment ("returns 1 if all of the provider's auth parameters are set, recording the missing ones when only some are") implies Bitbucket partial-configuration errors are reported. They are not.

`checkUserAndPasswordProvider` (lines 369-395) was extracted verbatim, so it still calls `GetEnvOrFile` twice per parameter in two passes. The new helpers also both take a `*strings.Builder` out-parameter and return an `int`, which is a two-channel result contract that makes each helper harder to test on its own.

Worked proposal:

```go
// missingParams returns the auth parameters of provider that are unset or blank.
func missingParams(provider string) []string { ... single pass ... }

func checkProvidersDefined() error {
    var errs []string
    configured := bitbucketDefined()

    for _, provider := range []string{providerNameGitHub, providerNameGitLab, providerNameGitea,
        providerNameSourcehut, providerNameAzureDevOps} { // or: providers table from P1-A
        params := enabledProviderAuth[provider]
        missing := missingParams(provider)
        switch {
        case len(missing) == 0:
            configured = true
        case len(missing) < len(params):
            for _, p := range missing { errs = append(errs, fmt.Sprintf("%s parameter '%s' is not defined.", provider, p)) }
        }
    }
    ...
}
```

This removes the out-parameter, the double scan, the loop-switch and the dead Bitbucket membership. One caveat must be preserved: at base, just-token providers report a parameter as an error only when it *exists but is blank*, not when it is absent. So `missingParams` needs to keep that distinction for that class, for example with a `blankParams` variant. Write a table test over `checkProvidersDefined` before collapsing, because none exists today.

## P3-A — Log-message typo and label drift are now centralised, so fix them

**Status: verified.**

`logProviderOrgs` (`internal/backup.go:203-207`) now holds the only copy of the format string `"%s Organistations: %s"`. At base the typo was repeated in three places. Now fixing it is a one-character edit, and the PR's own doc comment on the same helper spells "organisations" correctly. The same helper calls `strings.ToLower(orgs)` twice, once in the guard and once in the output, and lowercasing cannot change whether a string is empty, so the guard should just test `orgs != ""`. Together with the `"Gitlab"` LFS label noted in P1-A, these are the only places where "log output unchanged" fixes something that is plainly wrong. I recommend fixing them in the same change and noting it in the PR body.
