# Backup orchestration and startup configuration

## Scope and measurements

This subsystem covers the refactors in `internal/backup.go`: provider result collection, startup configuration logging, auth validation, and startup/scheduler orchestration. The file measured 729 lines at `main` and 774 lines at `review-head` (`git show main:internal/backup.go | wc -l`; `wc -l internal/backup.go`), a net increase of 45 lines. It remains well below the 1,000-line threshold. The patch’s overall backup.go hunk replaces 201 lines with 246, reflecting extracted helpers rather than a large expansion of the implementation.

Relevant evidence is at `collectProviderBackupResults` and the Bitbucket credential predicates (lines 95–138), startup logging helpers and provider displays (190–294), auth validation helpers (322–395), and `Run`, `scheduleBackups`, and `runScheduledJob` (397–541). `checkProvidersDefined` continues to special-case Bitbucket’s mutually alternative authentication modes (680–717).

## Assessment

No actionable finding. `collectProviderBackupResults` replaces repeated provider-specific credential checks and append calls with one compact function table, while retaining Bitbucket’s distinct two-mode credentials and preserving provider execution order. Extracting the two Bitbucket predicates removes duplicated checks from the backup and startup validation paths. The auth helpers make the two credential models explicit and preserve the previous partial-configuration error behavior.

The startup display functions have a small amount of repeated provider gating, but the shared log helpers now centralize formatting and default compare behavior. Keeping provider-specific setup visible also avoids hiding GitLab’s minimum-access-level and GitHub’s skip-user-repositories exceptions in a generic configuration engine. The `Run` decomposition isolates validation, working-directory creation, and scheduling. Interval and cron jobs now share registration/startup/shutdown handling through `runScheduledJob`; this is a useful reduction of duplicated orchestration rather than a pass-through wrapper.

## Worked code-judo alternatives considered

One possible further compression is a provider descriptor list holding each provider’s credential predicate, backup function, and startup logging fields, with Bitbucket represented by a predicate that accepts either auth mode. That would remove a few explicit provider calls, but it introduces a string-keyed registry and callback model for a fixed set of providers and makes GitLab and GitHub exceptions less direct. The current collection table is limited to the genuinely uniform token providers, so the larger registry is not a maintainability win.

The five `display*StartupConfig` functions could similarly be driven by descriptors. That would shorten the call site but would encode optional log lines, provider-specific labels, and GitLab’s default access level in configuration data or additional callbacks. The current short functions make both the credential gate and emitted settings easy to scan. No change is recommended.

## Verification status

The packet-authorized offline command `GOMODCACHE=<cache>/gomodcache GOCACHE=<cache>/gocache GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local go test ./internal/ -count=1` passed (`ok github.com/jonhadfield/soba/internal 5.227s`). `git diff --check main...review-head` passed. The clone remained clean after verification. Provider-backed integration behavior is not fully verified because credentials are unavailable; packet notes say those live tests skip.
