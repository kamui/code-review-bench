# 02 — `Run` decomposition and scheduling (`internal/backup.go`)

Scope: `Run`, `logRequestTimeout`, `validateStartupConfig`, `createWorkingDir`, `scheduleBackups`, `runScheduledJob`, and the package-global `job`.

## Measurements

- At base, `Run` was a single function of about 115 lines. At head, `Run` is 28 lines (`internal/backup.go:397-424`) and delegates to five helpers (lines 426-543).
- `grep -n "job" internal/backup.go` shows the package-level `var job gocron.Job` (line 737). It is read in `execProviderBackups` (line 35) and `runProviderBackups` (line 86) and written only in `runScheduledJob` (line 530).
- `go test ./internal/ -count=1` passes. The scheduled paths block on signals and are not exercised by tests.

## P1-C — `createWorkingDir` re-implements `resolveWorkingDir` a few dozen lines above it

**Status: verified by reading the head source.**

`createWorkingDir` (`internal/backup.go:473-486`) starts with:

```go
workingDIR := os.Getenv(envGitWorkingDir)
if workingDIR == "" {
    workingDIR = filepath.Join(backupDIR, workingDIRName)
}
```

This is exactly `resolveWorkingDir(backupDir)` (`internal/backup.go:140-146`), which `runProviderBackups` already uses to pick the directory it later cleans up. The PR gave the inline block its own named function, so it now sits as a sibling helper that duplicates an existing canonical one. The two definitions can drift, and if they do, the directory created at startup and the directory cleaned after each run would differ. That matters, because `cleanupWorkingDir` runs `os.RemoveAll`.

Remedy: `workingDIR := resolveWorkingDir(backupDIR)` and delete the comment and the three-line fallback.

## P2-B — `runScheduledJob` hides a write to package-global state; the scheduler is built even when unused

**Status: verified by reading the head source.**

`runScheduledJob(s, definition, options...)` (`internal/backup.go:525-543`) declares `var err error` only so it can assign the package-global `job` with `job, err = s.NewJob(...)`. `job == nil` is the program's only signal for "one-shot mode". `execProviderBackups` uses it to decide whether to `os.Exit(1)`, and `runProviderBackups` uses it to print the next-run banner. At base that assignment was visible inside `Run`. Now it is a side effect of a helper whose name and doc comment ("registers the backup task with the scheduler, starts it and blocks until shutdown") say nothing about publishing global mode state. A reader of `execProviderBackups` who wants to know where `job` is set now has to search for it.

`scheduleBackups` (lines 491-521) also still calls `gocron.NewScheduler()` before the `switch`, so the one-shot `default:` path creates a scheduler it never starts or shuts down. Both scheduled arms also pass the same `gocron.WithSingletonMode(gocron.LimitModeReschedule)` option. That option is policy the helper owns and could apply itself.

Worked proposal:

```go
func scheduleBackups() error {
    def, opts, desc := backupJobDefinition()   // nil def ⇒ one-shot
    if def == nil {
        execProviderBackups()
        return nil
    }
    logger.Print(desc)

    s, err := gocron.NewScheduler()
    if err != nil { return errors.Wrap(err, "failed to create scheduler") }

    j, err := s.NewJob(def, gocron.NewTask(execProviderBackups),
        append(opts, gocron.WithSingletonMode(gocron.LimitModeReschedule))...)
    if err != nil { return errors.Wrap(err, "failed to create job") }

    job = j // the single, visible write of scheduled-mode state
    s.Start()
    waitForShutdown(s)
    return nil
}
```

This removes one helper, the duplicated option, and the wasted scheduler in one-shot mode, and keeps the global write in the orchestration function where readers look for it. A longer-term improvement is to pass "scheduled" into `execProviderBackups` through a closure (`gocron.NewTask(func(){ execProviderBackups(true) })`), which removes the global entirely. That is beyond this PR's scope but is the obvious next step.

## P3-B — `validateStartupConfig` mixes validation with normalisation and reads the backup dir differently from everything else

**Status: verified. The inconsistency is pre-existing, and the extraction makes it more visible.**

`validateStartupConfig` (`internal/backup.go:443-471`) reads `envGitBackupDir` with `os.LookupEnv`. `displayStartupConfig` (line 191) and `runProviderBackups` (line 41) read the same variable with `GetEnvOrFile`, which also supports the `_FILE` indirection. So `GIT_BACKUP_DIR_FILE` would pass the per-run lookup but fail startup validation. The helper also returns a `TrimSuffix`-normalised directory, so despite its name it is the canonical source of `backupDIR` for `createWorkingDir`. This is not new behaviour. Now that validation has its own function, it is a good moment to make it read the variable the same way the rest of the code does, or to write down why startup deliberately does not honour `_FILE`.
