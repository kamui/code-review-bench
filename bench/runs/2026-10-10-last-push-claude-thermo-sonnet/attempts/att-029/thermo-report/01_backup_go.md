# 01 internal/backup.go

Commands: `git diff main...review-head -- internal/backup.go`; `wc -l internal/backup.go` (774 lines); `go test ./internal/ -count=1` (ok, 5.2s, offline).
Verification status: all findings verified by reading head source; no behaviour executed beyond the passing unit tests.

## A. checkProvidersDefined (finding 1)

Evidence: internal/backup.go:680-715 and internal/backup.go:101.

Worked code-judo proposal:

    bitbucketDefined := bitbucketAPITokenDefined() || bitbucketOAuthDefined()
    count := 0
    if bitbucketDefined { count++ }
    for provider := range enabledProviderAuth {
        if provider == providerNameBitBucketAPIToken || provider == providerNameBitBucketOAuth { continue }
        n, err := checkProvider(provider) ...
    }

This deletes the switch, the flag and the order-dependent comment. Pre-PR behaviour with both Bitbucket sets complete depended on map order (API-first: 1; OAuth-first: 2); the head is deterministic at 1.

## B. Startup config (finding 2)

Evidence: internal/backup.go:190-296 (five display functions, four helpers) and 106-114 (tokenProviders).

Proposal: define `type providerInfo struct{ label, tokenEnv, orgsEnv, backupsEnv, compareEnv, lfsEnv string; run func(string) *ProviderBackupResults }` and a package-level `[]providerInfo`. `collectProviderBackupResults` and `displayStartupConfig` both range over it; empty env fields are skipped. The GitHub skip-user-repos and GitLab min-access-level lines become small hooks. Typo 'Organistations' appears once in `logProviderOrgs` and the 'Gitlab' LFS label inconsistency (internal/backup.go:~272) should be unified, which changes log output slightly and so should be called out.

## C. createWorkingDir (finding 3)

Evidence: internal/backup.go:473-486 vs resolveWorkingDir at 139-144. Proposal: `workingDIR := resolveWorkingDir(backupDIR)` then log and `MkdirAll`.

## D. checkProvider helpers (finding 4)

Evidence: internal/backup.go:348-404. Proposal: `func authParamSet(p string) bool { v, ok := GetEnvOrFile(p); return ok && strings.TrimSpace(v) != "" }` (note original used Trim(val, " "), so only spaces; TrimSpace would widen slightly) and compute `missing` in one pass.
