# Backup and startup orchestration

## Finding status

No actionable findings for `internal/backup.go`.

## Evidence and measurements

The committed change refactors `runProviderBackups`, `displayStartupConfig`, `checkProvider`, `Run`, and `checkProvidersDefined`. The current file is 774 lines, up from 729 at the merge-base; the change does not cross the 1,000-line threshold. The main flows now delegate provider collection, validation, working-directory creation, and scheduled-job setup into named helpers.

In `collectProviderBackupResults`, the provider order remains Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, and Sourcehut. Bitbucket's API-token and OAuth completion checks remain separate because they have different credential shapes. The list of token-only providers is represented as a local slice of environment key and runner pairs; this removes repeated dispatch branches without introducing package-level registration state.

The startup display helpers preserve each provider's gate and settings. Common formatting is centralized in `logProviderOrgs`, `logProviderBackupsToKeep`, `logProviderCompareMethod`, and `logProviderBackupLFS`; GitHub's skip-user-repositories setting and GitLab's minimum access-level default remain provider-specific. `Run` calls timeout logging, startup validation, working-directory creation, and scheduling in the previous order. `scheduleBackups` retains the interval-before-cron precedence and one-shot fallback; the two scheduled variants share the job registration and shutdown flow.

Provider validation is split into token-only and username/password checks. The token-only path still reports a defined but blank parameter and counts every nonblank parameter. The paired-credential path still reports missing fields only for partial configurations and counts a provider only when all fields are present. `checkProvidersDefined` uses the extracted Bitbucket completeness predicates while retaining the one-method-count behavior when both credential styles are configured.

## Code-judo assessment

A possible further reduction is to describe startup display settings with a provider metadata table and a generic renderer. That would replace several small provider functions with configuration data, but it would need extra fields or callbacks for GitHub's skip-repositories flag and GitLab's default access level. It would also make output behavior depend on interpreting a data structure rather than reading each provider's concise function. Given the current five providers and explicit exceptions, the table would move complexity rather than delete it. The current decomposition is the simpler model.

Likewise, combining validation helpers into a single generic credential checker would obscure the distinct completeness rules for a token versus paired credentials. No cleaner shared state model is apparent from this diff.

## Verification status

`go test ./internal/ -count=1` passed under the supplied offline cache configuration. The test package can skip tests requiring provider credentials; those live paths were not independently exercised. `git diff --check` produced no output. No tests were added or changed by the reviewer.

## Remediation

None requested. Preserve the current explicit helper boundaries unless the provider set grows enough to justify a measured redesign.
