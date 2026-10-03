# Backup startup, working directories, and scheduling

## Finding: Reuse the working-directory resolver during startup creation

In `internal/backup.go:473–478`, the extracted `createWorkingDir` duplicates the environment override and default-path selection already owned by `resolveWorkingDir` at lines 140–146. `runProviderBackups` uses that existing resolver to select the directory passed to cleanup, while startup creation now exposes a second implementation of the same policy. The extraction misses a direct opportunity to remove duplicated lifecycle logic and keeps creation and cleanup dependent on separately maintained selection rules. Set `workingDIR := resolveWorkingDir(backupDIR)` inside `createWorkingDir`, retaining its logging, `filepath.Clean`, permissions, and error wrapping. This reuses the canonical helper with identical current behavior; it does not require a new abstraction or a broader configuration rewrite.

## Source evidence and ownership

`runProviderBackups` selects a working directory through `resolveWorkingDir(backupDir)` at `internal/backup.go:52`, then defers `cleanupWorkingDir(workingDir, backupDir)`. The resolver at lines 140–146 uses a non-empty `GIT_WORKING_DIR` override, otherwise `filepath.Join(backupDir, workingDIRName)`.

`Run` now calls the extracted `createWorkingDir(backupDIR)` at lines 417–419. That helper repeats exactly the same override and join at lines 475–478 before logging and `os.MkdirAll`. The policy was already duplicated inline in the base. The extraction gives it a named lifecycle boundary but fails to reuse the existing canonical utility. This is directly in the changed function and has an exact, small remedy; no hypothetical runtime failure is needed to justify deleting duplicate policy.

The existing resolver is package-local, already in the same file, and already accepts the needed argument. Reuse requires no new type, optional parameter, dispatcher, or caller change.

## Worked replacement

```go
func createWorkingDir(backupDIR string) error {
    workingDIR := resolveWorkingDir(backupDIR)
    logger.Println("creating working directory:", workingDIR)

    if mkErr := os.MkdirAll(filepath.Clean(workingDIR), workingDIRMode); mkErr != nil {
        return errors.Wrap(mkErr,
            fmt.Sprintf("failed to create working directory %q", workingDIR))
    }
    return nil
}
```

The default selection and override match the head exactly. Keep cleaning at the filesystem operation so displayed and error-reported values remain as before. Keep `workingDIRMode` and the error message. The remedy does not change cleanup safeguards, directory validation, or the environment source used by startup configuration.

| Input | Existing resolver and extracted creator both select |
| --- | --- |
| Override unset | `filepath.Join(backupDIR, ".working")` |
| Override explicitly empty | The same default path |
| Override non-empty | Its raw environment value |

The proposal is not applied or compiled. Existing tests do not explicitly characterize working-directory override creation and cleanup through this boundary. A remedy can be checked with default and override cases without contacting providers.

## Other lifecycle changes and assessment

The head `Run` at lines 397–422 keeps the old startup ordering: resolve git, display config, log executable and version, parse/log request timeout, validate backup directory and providers, create the working directory, then select scheduling. `validateStartupConfig` retains `os.LookupEnv` for the backup directory, the GitHub organisations/token presence dependency, one-newline suffix trimming, stat errors, and wrapped provider-validation errors. Moving these checks into a returned `(string, error)` boundary makes the startup flow easier to read. No extra wrapper criticism is warranted where the helper owns a coherent operation.

`logRequestTimeout` both validates and logs, as its comment now states. Parsing failure still occurs before backup-directory validation. `createWorkingDir` retains directory creation after successful provider validation. The review does not request a global immutable configuration snapshot because rereading provider environment values on later runs is existing behavior and changing it would exceed a behavior-preserving restructuring.

`scheduleBackups` at lines 491–521 preserves interval precedence over cron, duration conversion from minutes, immediate start only for interval jobs, and direct one-shot execution when neither mode is configured. Scheduler creation still happens before the switch, including one-shot mode. That pre-existing allocation and lifetime is not an introduced regression and is not raised as an actionable finding.

`runScheduledJob` at lines 525–543 earns its abstraction: it removes two copies of job creation, task binding, error wrapping, scheduler start, and signal-wait orchestration. The options retain singleton mode in both scheduled branches. The helper writes the same package-level `job` before scheduler start; changing that variable requires accounting for the existing next-run log and one-shot exit decision, so no unsupported local-variable cleanup is proposed.

`waitForShutdown` and the cleanup safety checks are unchanged. No scheduler run was started during the review, because those tests would block until process signals and the existing package tests do not exercise that path. Static comparison establishes the retained calls and options, not a runtime verification of signal delivery or shutdown races.

## Measurements and verification

The committed diff was inspected with `git diff main...review-head -- internal/backup.go`; both complete versions were read. The whole file grows 45 lines and adds 19 top-level functions. It remains below 1,000 lines. The function extraction improves local startup and scheduling cohesion, so line growth alone is not a finding. No strict-size waiver or author question is needed.

The offline internal suite passed, including missing-backup-directory, GitHub config dependency, timeout parsing, and no-provider cases. The mock GitHub host test does not call `Run` or `collectProviderBackupResults`, so it cannot be cited as scheduler or orchestration verification. The directory-resolver proposal remains a source-verified equivalence only. Full suite output is retained outside the checkout.
