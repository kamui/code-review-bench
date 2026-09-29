# Backup flow and startup configuration

## Scope and measurements

I reviewed the `internal/backup.go` changes in `main...review-head`, focusing on provider dispatch, startup configuration logging, credential validation, and scheduling. The file changed from 729 to 774 lines (+45), remaining below the skill's 1,000-line decomposition threshold. The central `runProviderBackups`, `displayStartupConfig`, `checkProvider`, and `Run` functions now delegate discrete operations to named helpers.

## Findings

No actionable structural finding. `collectProviderBackupResults` makes the provider execution order and credential gate visible, and keeps Bitbucket's two alternative credential schemes explicit rather than forcing them into a misleading uniform model. The small token-provider table removes repeated dispatch conditionals while preserving a concrete ordered list. The Bitbucket predicates are reused by configuration validation, avoiding duplicated tests for those two credential combinations.

`displayStartupConfig` now delegates per-provider output to provider-specific functions, while the shared logging helpers centralize repeated formatting. This is a modest, legible decomposition: provider-specific exceptions such as GitHub's skip-user-repos setting and GitLab's minimum access level remain local. I considered whether the additional helper count merely relocates complexity; the new provider functions group related settings and the common helpers remove repeated formatting branches, so there is no clear simpler restructuring that deletes meaningful complexity without obscuring provider-specific behavior.

`checkProvider` delegates the two credential shapes to separate functions. Those helpers preserve the prior distinction between independent token credentials and complete user/password-style credentials, including the partial-configuration errors. `Run` now reads as a startup sequence, and `scheduleBackups` centralizes the shared scheduled-job lifecycle through `runScheduledJob`; interval and cron options remain visible at their call sites. These helpers have clear responsibilities and do not create a new state model or spread feature checks into unrelated paths.

## Verification status

The permitted offline command `go test ./internal/ -count=1` passed (`ok`, 5.173s). Review of the diff found no material behavior drift in these extracted paths. The run did not execute provider-credential-dependent live backup cases because credentials are unavailable.

## Worked code-judo assessment

A more ambitious provider registry could unify credential metadata, validation, logging, and execution, but that would couple distinct provider configuration shapes and output policies into a generic mechanism. The current change's explicit exceptions are simpler to scan and make the real differences visible. Keeping the scheduler's small shared registration helper likewise avoids duplicating job creation and shutdown handling. No further restructuring is justified by this diff.
