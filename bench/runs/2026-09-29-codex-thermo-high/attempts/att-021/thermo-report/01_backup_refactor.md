# Backup flow refactor

## Scope and measurement

Reviewed `internal/backup.go` across `main...review-head`, with attention to provider selection, startup logging, credential validation, and the `Run`/scheduler lifecycle. The file is 774 lines after the change, below the skill's 1,000-line threshold. `git diff --check` reported no whitespace errors.

## Findings and evidence

No actionable finding. `runProviderBackups` now delegates selection to `collectProviderBackupResults` (lines 95–138). The ordered token-provider list follows the same order as the original sequence: Gitea, GitHub, GitLab, Azure DevOps, then Sourcehut. Bitbucket remains first and is enabled by either of the same complete credential pairs. The extracted predicates are also reused by provider validation, removing duplicated checks rather than introducing another source of truth.

`displayStartupConfig` delegates to provider-specific functions and a small set of shared logging operations (lines 190–294). The provider guards and logged options remain explicit, which keeps provider differences visible. `checkProvider` delegates the two existing authentication models (lines 322–395); blank-value handling and partial-credential errors remain attached to the same model. `Run` now reads as the existing startup sequence—timeout, configuration validation, work-directory creation, then scheduling (lines 397–541). The scheduler helper retains job registration, start, and shutdown waiting in that order.

## Code-judo assessment

A table-driven startup logger could reduce some repeated provider call sites, but would encode optional provider-specific settings and a GitLab default in a generic descriptor model. That trades direct control flow for a more indirect data structure without deleting meaningful complexity. Likewise, folding authentication validation into a generic credential abstraction would need to preserve the distinct “any nonblank token” and “all fields required together” rules. The current small helpers expose those rules more clearly.

The provider result list is a useful, bounded data-driven loop: it removes repeated credential-gate/append branches while keeping the exceptional Bitbucket authentication path separate. The scheduler wrapper centralizes lifecycle steps shared by both schedules and avoids duplicating registration/start/wait code. I found no better structural rewrite with a clear maintainability gain.

## Verification status and remediation

Static diff and surrounding-code inspection only; no tests or build commands were run. No remediation is suggested. The test and Docker execution constraints do not affect this subsystem assessment.
