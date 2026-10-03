# Thermo-nuclear review of soba #195

Reviewed `c77f548cbd340d2e744da7c6d92a54372b4b900b..136a4850df9aec8cf8813b27ba1533d4a17bc642`, using the committed `main...review-head` diff and the frozen review skill. This is one primary review context; no independent workers, alternate models, upstream discussions, or external reference answers were used. The checkout was not edited.

## Verdict

Request changes under the selected strict maintainability bar. There are three actionable findings: the credential extraction retains avoidable orchestration and introduces an awkward output contract, the new directory-creation helper duplicates an existing resolver, and the Gitea assertion extraction misses an opportunity to replace duplicated scenario logic with expected data. These are maintainability findings, not demonstrated backup failures. The observed refactor preserves the important runtime branches, and the permitted offline tests passed.

The notification title extraction is a useful abstraction: three identical switches become one policy function. The provider dispatch table preserves order and replaces repeated token checks with a direct loop. Scheduler extraction preserves interval precedence, singleton scheduling, immediate interval execution, and shutdown waiting. These changes do not need further abstraction.

## Credential validation still moves complexity between helpers

In `internal/backup.go:326–395`, `checkProvider` now dispatches to two helpers that return a count while mutating its caller-owned `*strings.Builder`. The user/password helper still scans every credential to count it and then reads every credential again to reconstruct missing-parameter diagnostics. This extraction expands the validator from 49 to 70 lines and makes its result depend on two output channels without removing the repeated read/format flow. Since `GetEnvOrFile` can open files and log failures, the second pass is more than a repeated map lookup. Replace the two builder-mutating helpers with one credential scan that records valid and missing parameters, applies the existing token-versus-all-required policy, and returns the count and error together. Preserve the existing distinction between absent and explicitly blank token settings and the existing diagnostic wording. [Full evidence and a worked replacement](01_backup.md#credential-validation).

## The new working-directory helper bypasses the canonical resolver

In `internal/backup.go:473–478`, the new `createWorkingDir` helper repeats the `GIT_WORKING_DIR` lookup and default-path construction already owned by `resolveWorkingDir` at lines 140–146. Backup cleanup uses that resolver, while startup directory creation keeps a second implementation of the same choice. A future change to the default or override rules would therefore require synchronized edits to creation and cleanup. Initialize `workingDIR` with `resolveWorkingDir(backupDIR)` and leave logging, cleaning, permissions, and error wrapping in `createWorkingDir`. This removes the duplicated policy without changing current path selection. [Full evidence and the small replacement](01_backup.md#working-directory-resolution).

## Gitea assertions duplicate a scenario instead of modeling its expected tree

In `internal/backup_test.go:651–681`, `assertGiteaOrgTwoOnlyBackedUp` and `assertGiteaAllOrgsBackedUp` repeat the same org-two directory read, cardinality assertion, and two repository-prefix assertions. The caller still switches on the organization selector to choose between these helpers. The extraction removes local nesting but leaves the common scenario checks in two places, so adding another selection case requires another assertion body or further helper nesting. Put the selector and expected organization-to-repository-prefix map in a table, and use one assertion helper to compare both known organization directories against that map, including the absent-org assertion. Keep the exact entry counts, timestamp-tolerant prefix checks, and per-iteration cleanup. This deletes the scenario switch and both overlapping assertion bodies while preserving the current checks. [Full evidence and a worked table-driven form](02_backup_tests.md#gitea-organization-scenarios).

## Measurements and verification

| Changed file | Base lines | Head lines | Assessment |
| --- | ---: | ---: | --- |
| `internal/backup.go` | 729 | 774 | Adds 19 top-level functions, from 23 to 42; review the quality of boundaries rather than the count alone. |
| `internal/backup_test.go` | 1016 | 989 | Shrinks below 1,000 lines; no threshold-crossing finding. |
| `internal/notify.go` | 248 | 238 | Removes repeated status policy. |
| `docker/Dockerfile` | 23 | 22 | Local layer and quoting cleanup. |

The permitted `go test ./internal/ -count=1 -timeout=4m -v` run passed in 5.227 seconds: 24 top-level tests passed, 15 skipped, and none failed. The run used the attempt's dependency and build caches, `GOFLAGS=-mod=mod`, `GOPROXY=off`, and `GOTOOLCHAIN=local`, with a five-minute command limit. `GIT_BACKUP_DIR` was unset for the test command so the suite created its own temporary backup directory. [Captured output](../internal-test.log).

Live provider credentials and Docker were unavailable. In particular, the changed Gitea organization test skipped; its assertion preservation was checked by source comparison. The local GitHub fixture test passed, but it calls the host directly and does not verify the new provider dispatcher. The test run does not establish scheduler/shutdown equivalence or exercise notification delivery. These paths were inspected statically. The worked remedies are review proposals and were not applied or executed. See the detail reports for the limits of each conclusion.

No build, vet, lint, Docker build, full-repository test run, or live-provider run was performed. The PR author's reported checks are not treated as independent verification. The initial and post-test worktree checks were clean.

## Remediation sequence

First replace the credential output contract and eliminate its repeated scan, preserving the documented cases in the backup detail report. Next reuse `resolveWorkingDir` during creation; this is a small independent fix. Then consolidate the Gitea scenario assertions around expected data and retain a cleanup path for assertion failures. Rerun the offline internal tests after implementation, and execute the live Gitea scenario when its fixture credentials are available. Characterize the credential-policy cases directly because the existing offline suite does not cover that matrix.

The existing startup helpers and scheduler lifecycle helper can remain. A broad provider framework or concurrency change would add scope without a demonstrated benefit here. No file-size blocker, new unsafe cast, new optional mode, or newly serialized independent operation was found.

## Detail reports

- [Backup orchestration, credential validation, startup logging, and scheduling](01_backup.md)
- [Backup-test deletion and Gitea assertion structure](02_backup_tests.md)
- [Notification titles and Dockerfile](03_notify_docker.md)

There are no unresolved review questions.
