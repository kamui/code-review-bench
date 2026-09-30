# 02 — `Run` decomposition and scheduling (`internal/backup.go`)

Scope: `Run`, `logRequestTimeout`, `validateStartupConfig`, `createWorkingDir`, `scheduleBackups`, `runScheduledJob`, and the package-global `job` (backup.go:737).

Commands used:

```
git diff main...review-head -- internal/backup.go
grep -n "job\b" internal/*.go | grep -v _test
```

## Overall

The top-level `Run` now reads as a clear pipeline: log git → log timeout → validate → create working dir → schedule. This is a real legibility improvement. `validateStartupConfig` also collapses the nested `if ghOrgsExists { if !githubTokenExists {…} }` into one condition, which is good. The remaining concern is about hidden state, not complexity.

## Finding S1 — `runScheduledJob` hides a write to the package-global `job` that `execProviderBackups` depends on

**Where:** `internal/backup.go:525-542` (helper), `internal/backup.go:30-38` and `internal/backup.go:87-90` (readers), `internal/backup.go:737` (`var job gocron.Job`).

**Evidence.** `runScheduledJob` declares `var err error` only so it can write `job, err = s.NewJob(...)`, assigning the package global. That global is load-bearing:

- `execProviderBackups` uses `job == nil` to decide whether it is a one-shot run and should call `os.Exit(1)` on failure (backup.go:35).
- `runProviderBackups` uses `job != nil` to print the next-run banner (backup.go:87).

Before the PR, the assignment sat visibly in `Run`, next to the `switch` whose `default:` branch relies on `job` staying nil. After the PR it is buried two calls deep inside a helper whose name ("run scheduled job") and doc comment ("registers the backup task… starts it and blocks") don't mention it. A reader of `scheduleBackups` can no longer tell that the one-shot branch and the scheduled branches differ in global state. That is exactly the kind of implicit coupling a decomposition should remove, not bury.

A secondary point: both call sites pass `gocron.WithSingletonMode(gocron.LimitModeReschedule)`, so the helper takes a variadic option list it doesn't need.

**Remedy.** The minimal fix is to make the side effect explicit in the helper's contract: document it, or better, return the job and assign it in `scheduleBackups`:

```go
func scheduleBackups() error {
    ...
    switch {
    case backupInterval != 0:
        logger.Printf("scheduling to Run every %s", formatIntervalDuration(backupInterval))
        return startAndWait(s, gocron.DurationJob(time.Duration(backupInterval)*time.Minute),
            gocron.WithStartAt(gocron.WithStartImmediately()))
    case backupCron != "":
        ...
        return startAndWait(s, gocron.CronJob(backupCron, false))
    default:
        execProviderBackups()
        return nil
    }
}

func startAndWait(s gocron.Scheduler, def gocron.JobDefinition, opts ...gocron.JobOption) error {
    opts = append(opts, gocron.WithSingletonMode(gocron.LimitModeReschedule))
    j, err := s.NewJob(def, gocron.NewTask(execProviderBackups), opts...)
    if err != nil {
        return errors.Wrap(err, "failed to create job")
    }
    job = j // read by execProviderBackups / runProviderBackups to detect scheduled mode
    s.Start()
    waitForShutdown(s)
    return nil
}
```

The more ambitious move is to stop using the global as a mode flag. Pass a `scheduled bool` (or the `gocron.Job`) into `execProviderBackups` through a closure (`gocron.NewTask(func() { execProviderBackups(j) })`). The one-shot/scheduled distinction then becomes a parameter rather than ambient state. This change touches `resetGlobals` in tests and is out of scope for a Sonar cleanup, but it is the direction in which the global disappears.

**Verification status:** Confirmed by reading. The ordering is unchanged: under `WithStartImmediately` the job still runs only after `s.Start()`, which happens after `job` is assigned, exactly as before. No behavioural regression. The finding is about legibility and hidden coupling.

## Note (no finding)

`logRequestTimeout` calls `getRequestTimeout` only to log and validate. `getHTTPClient` calls it again (backup.go:574) and ignores the error. This duplication is pre-existing and harmless, and I'm leaving it out of the findings.
