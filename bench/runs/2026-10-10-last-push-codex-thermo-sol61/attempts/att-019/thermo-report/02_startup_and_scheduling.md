# Startup, directory ownership, and scheduling

## Evidence and finding F2

Reviewed the full base/head `internal/backup.go`, including unchanged cleanup
and logging setup, using `git diff main...review-head`, `git show
main:internal/backup.go`, and `nl -ba internal/backup.go`.

The new `createWorkingDir` at lines 473–487 duplicates a complete policy:
read `GIT_WORKING_DIR`; use it if nonempty; otherwise join the backup directory
and `.working`. `resolveWorkingDir` at lines 140–146 already implements
that exact selection. `runProviderBackups` uses it at line 52 and passes its
result into deferred `cleanupWorkingDir` at line 57.

Directory creation and cleanup selection should share this one resolver.
The duplication was inline in `Run` at the base, so the review does not claim
that the PR introduced the path-selection rule. It introduces a dedicated
helper around the duplicate despite the canonical helper already existing.
That is a concrete missed reuse opportunity under this review's approval bar.
A future change to override/fallback handling would otherwise need to update
two implementations to ensure creation and cleanup select the same directory.

## Worked code-judo proposal

Replace the new helper's local selection branch with the existing resolver.
All filesystem effects and operational messages remain in their current
layer. No additional abstraction is needed.

```go
func createWorkingDir(backupDIR string) error {
    workingDIR := resolveWorkingDir(backupDIR)
    logger.Println("creating working directory:", workingDIR)

    if mkErr := os.MkdirAll(filepath.Clean(workingDIR), workingDIRMode); mkErr != nil {
        return errors.Wrap(mkErr, fmt.Sprintf("failed to create working directory %q", workingDIR))
    }
    return nil
}
```

For a nonempty override both implementations return exactly that string.
For an empty or unset override both return `filepath.Join(backupDIR,
workingDIRName)`. Keeping `filepath.Clean` at the creation call also retains
the existing distinction between the selected path printed in logs and the
cleaned path passed to `MkdirAll`. The same mode and error text remain.

The proposal is a static review sketch. It was not applied or tested. Its
equivalence follows directly from the identical two-case selection rules.
Unifying backup-directory configuration reads or altering cleanup containment
checks would be separate behavioral work and is not part of this finding.

## Startup decomposition assessment

`Run` now composes git discovery, startup logging, request-timeout validation,
configuration validation, working-directory creation, and scheduling. The
error wrapping and order match the base. These are recognizable operations,
so the extraction is useful rather than a series of identity wrappers.

`validateStartupConfig` still uses `os.LookupEnv` for the backup directory,
requires the GitHub token key if the organisations key exists, removes one
trailing newline from the directory, checks its filesystem accessibility,
and wraps provider-validation failures. File-backed backup-directory handling
is inconsistent elsewhere in the existing code, but this PR simply retains
that boundary. It is not reported as a newly introduced defect.

The five provider-specific startup functions have explicit credential gates
and meaningful provider-specific ordering. Shared logging helpers remove
repeated formatting and compare-method defaults. GitHub's skip-user flag
and GitLab's minimum-access-level message remain explicit. A generic field
printer with optional hooks would add machinery to describe these differences
and is not required here. The existing Gitlab LFS label spelling is retained.

Changing the Bitbucket comparison check from lowercasing to `EqualFold` is
equivalent for the ASCII target `refs`. Neither branch trims whitespace,
which remains unchanged. `envTrue` still reads direct environment variables;
the surrounding `GetEnvOrFile` check does not newly enable file-backed boolean
values. That limitation predates the change and is not raised as a finding.

The logger is initialized at line 660 with `log.Lshortfile|log.LstdFlags`.
Moving a `Printf` into a helper changes its source-location metadata. Shared
helpers also make several providers report the same caller line. This is
the factual basis for Q1, rather than an assertion that extraction is wrong.

## Scheduling assessment and behavior boundaries

`scheduleBackups` preserves interval priority over cron and one-shot behavior
when neither is configured. Interval registration retains singleton mode
and start-immediately. Cron retains singleton mode without immediate start.
The `runScheduledJob` helper receives the actual job definition and options,
registers `execProviderBackups`, assigns the same package-level `job`, wraps
creation errors, starts the scheduler, and waits for shutdown.

This removes two copies of the same task setup and lifecycle. It is a useful
abstraction: the variable job definition/options stay visible at the caller,
while the shared task/lifecycle stays together. No new flags or magic dispatch
mechanism are needed. Returning directly from each scheduled branch is
equivalent to the previous common `return nil` after shutdown.

The one-shot branch still creates a scheduler before invoking backups, and
the package-level job still affects exit behavior and next-run messages.
Those ownership questions exist at the base. This review does not attribute
them to the PR or demand a scheduler redesign to fix the two findings.

No scheduler integration or signal-shutdown test was executed. The existing
internal test pass verifies compilation of the extracted gocron API calls,
but not actual cron execution, startup timing, or shutdown handling.
