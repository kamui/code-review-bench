# Thermo-nuclear review of soba #195

## Verdict

Request changes under the selected maintainability standard. The refactor makes
the top-level functions easier to scan, and the notification-title extraction
removes duplication cleanly. However, the provider refactor stops at extracting
helpers when an explicit provider model could remove the parallel registries and
dispatch machinery. There is also a small, concrete duplication of an existing
path resolver in the newly extracted startup helper.

These are two maintainability findings, not confirmed runtime regressions. The
permitted internal package test run passed. Passing tests do not establish
equivalence for the credential-dependent backup and scheduling paths that were
not exercised.

## Scope and evidence

Reviewed the committed range
`c77f548cbd340d2e744da7c6d92a54372b4b900b..136a4850df9aec8cf8813b27ba1533d4a17bc642`
with `git diff main...review-head`, all four changed files, and their local
provider/configuration callers. This was one primary review context, without
delegation or alternate-model review. Repository guidance and prior review
comments were not used as review instructions or finding evidence. No network
research was performed.

| File | Base lines | Head lines | Assessment |
| --- | ---: | ---: | --- |
| `internal/backup.go` | 729 | 774 | Adds 19 functions; configuration policy remains split |
| `internal/backup_test.go` | 1,016 | 989 | Removes a duplicate test and simplifies assertions |
| `internal/notify.go` | 248 | 238 | Shared title policy is a useful abstraction |
| `docker/Dockerfile` | 23 | 22 | Small, coherent shell/layering changes |

No file crosses upward through 1,000 lines. The changed test file drops below
that threshold. The runtime file's growth alone is not a finding.

## Actionable findings

### Consolidate provider policy instead of preserving parallel registries

In `internal/backup.go:105–114`, the new `tokenProviders` table centralizes backup execution, but provider policy still resides separately in `enabledProviderAuth`, `justTokenProviders`, and `userAndPasswordProviders` in `internal/constants.go:120–159`. The extracted `checkProvider` path at `internal/backup.go:326–395` still dispatches through those lists and passes a mutable error builder into helpers whose integer results mean different things: populated parameters for Gitea/Sourcehut and complete credential tuples for Azure DevOps. A provider change therefore requires coordinating selection, validation parameters, and validation policy across independent declarations. This split predates the PR, but the new table and helper extraction preserve it instead of completing the structural simplification. Define one ordered, typed descriptor for each ordinary provider containing its execution gate, validation parameters, validation policy, and backup function; use it for both collection and startup validation. Keep Bitbucket's alternative authentication methods explicit. This lets the parallel classification lists and string-based dispatch disappear while preserving the existing distinction between a configuration that counts toward startup validation and credentials that trigger execution. Full evidence and a worked replacement are in [01_backup_runtime.md](01_backup_runtime.md).

### Reuse the existing working-directory resolver

In `internal/backup.go:473–478`, the new `createWorkingDir` helper repeats the `GIT_WORKING_DIR` lookup and `.working` fallback already owned by `resolveWorkingDir` at lines 140–145, which backup execution uses at line 52 to choose the directory passed to cleanup. The extraction makes this duplicated policy a separate named startup operation without giving it a shared source of truth. A later change to the fallback or environment handling can make startup create one directory while execution selects another for cleanup. Initialize `workingDIR` with `resolveWorkingDir(backupDIR)` and retain the existing logging, path cleaning, permissions, and wrapped error. The two implementations currently agree; this is a canonical-helper reuse finding, not an observed path failure. The exact replacement and equivalence checks are in [01_backup_runtime.md](01_backup_runtime.md).

## Verification and limitations

`git diff --check main...review-head` passed. The internal package suite passed
with `-count=1 -v -timeout=240s`, the supplied module/build caches,
`GOFLAGS=-mod=mod`, `GOPROXY=off`, and `GOTOOLCHAIN=local`; package runtime was
5.205 seconds. The full command is recorded in
[02_backup_tests.md](02_backup_tests.md).

The run was not entirely offline: the existing invalid-token GitHub test opened
an external connection and received a 401 response. No upstream discussions or
review material were fetched. The local GitHub fixture test also passed. Most
credential-dependent tests, including the changed Gitea organization test,
skipped. No additional package run was performed.

The local GitHub fixture test calls `githosts.NewGitHubHost` directly, so its
success does not verify the new provider collection loop. No tests directly
exercise the extracted scheduler helper, all startup log combinations, or the
shared notification-title function. Behavior in those areas was compared by
source inspection. Docker is unavailable, so the image was not built. The PR's
claimed build, vet, lint, and complete credentialed suite results were not
independently reproduced.

The checkout remained clean after testing. Head tree identity is
`d50974711ad3921de8e81016e044188ad8bd0358`; no remedies were applied.

## Remediation sequence

First put provider selection and validation metadata in one descriptor, retain
the current parameter-presence rules, and remove the obsolete map/list dispatch.
Keep execution sequential and retain the current provider order. Add focused
behavioral checks for absent, blank, partial, and complete credentials; include
Gitea/Sourcehut API-URL-only configurations and both Bitbucket authentication
methods so the redesign does not silently tighten existing behavior.

Then replace the duplicated startup directory resolution with the existing
helper. This change is small enough to verify directly against the unset,
empty, and explicit-directory cases.

Keep the shared notification-title function and the duplicate-test removal.
The remaining subsystem assessments are preserved in
[02_backup_tests.md](02_backup_tests.md) and
[03_notifications_docker.md](03_notifications_docker.md).

## Questions

None. The source provides enough evidence for both findings without requesting
additional information.
