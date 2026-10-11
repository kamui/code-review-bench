# 02 — The `Run` decomposition (`internal/backup.go`)

Covers summary findings F1 and F7 and question Q1. All line numbers are at the head, `136a485`.

The PR splits `Run` into `logRequestTimeout`, `validateStartupConfig`, `createWorkingDir`, `scheduleBackups` and `runScheduledJob`. The resulting `Run` (`backup.go:397-422`) reads well as a sequence, and `logRequestTimeout` is a clean extraction. The other four are where the issues are.

## 1. `createWorkingDir` duplicates `resolveWorkingDir` (F1)

**Evidence.** `internal/backup.go:473-487`, new in this PR:

```go
func createWorkingDir(backupDIR string) error {
	// Check if GIT_WORKING_DIR is set, otherwise use default
	workingDIR := os.Getenv(envGitWorkingDir)
	if workingDIR == "" {
		workingDIR = filepath.Join(backupDIR, workingDIRName)
	}

	logger.Println("creating working directory:", workingDIR)
	// ...
```

`internal/backup.go:140-146`, already present at the base:

```go
func resolveWorkingDir(backupDir string) string {
	if w := os.Getenv(envGitWorkingDir); w != "" {
		return w
	}

	return filepath.Join(backupDir, workingDIRName)
}
```

`resolveWorkingDir` is called from `runProviderBackups` at line 52, and its result is handed to `cleanupWorkingDir`.

**Why this is a problem.** The two blocks are the same rule. One decides which directory startup creates; the other decides which directory each backup run uses and then deletes. They must agree, and the only thing making them agree is that they were typed the same way. The inline copy existed in `Run` before, so this is not a new duplication in the strict sense, but extracting the block into a named function without looking for the existing helper makes the duplication structural: there are now two functions in one file whose job is to know where the working directory is.

**Remedy.**

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

**Verification status.** Verified by reading (the two bodies are the same expression) and by the scratch build, vet and package tests. No test in the repository exercises `GIT_WORKING_DIR` on the startup path without live credentials.

## 2. `scheduleBackups` and `runScheduledJob` (F7)

**Evidence.** `internal/backup.go:491-541`:

```go
switch {
case backupInterval != 0:
	logger.Printf("scheduling to Run every %s", formatIntervalDuration(backupInterval))

	return runScheduledJob(s,
		gocron.DurationJob(time.Duration(backupInterval)*time.Minute),
		gocron.WithSingletonMode(gocron.LimitModeReschedule),
		gocron.WithStartAt(gocron.WithStartImmediately()),
	)
case backupCron != "":
	logger.Printf("scheduling to Run with cron '%s'", backupCron)

	return runScheduledJob(s,
		gocron.CronJob(backupCron, false),
		gocron.WithSingletonMode(gocron.LimitModeReschedule),
	)
default:
	execProviderBackups()
}

return nil
```

```go
func runScheduledJob(s gocron.Scheduler, definition gocron.JobDefinition, options ...gocron.JobOption) error {
	var err error

	job, err = s.NewJob(
		definition,
		gocron.NewTask(execProviderBackups),
		options...,
	)
```

`job` is the package variable declared at line 737. Its readers are `execProviderBackups` (line 35, which calls `os.Exit(1)` on failure only when `job == nil`) and `runProviderBackups` (line 87, which logs the next run).

**Why this is a problem.** The extraction removed the duplicated `NewJob` / `Start` / `waitForShutdown` tail, which is good. But the cut is around the wrong thing.

The helper's name says "run a scheduled job". What it also does is set process-wide state that changes how the task it registers behaves on failure. Before the PR that assignment was in `Run`, in plain view. Now a reader of `scheduleBackups` sees no mention of `job` at all, and a reader of `runScheduledJob` has to notice that `var err error` on its own line is there so the next line assigns a global.

The helper's parameter list is `(scheduler, definition, options...)`, which is the parameter list of `s.NewJob` minus the task. It is a forwarding wrapper with a fixed third step.

The `switch` has two arms that return a value and one that does not, and `WithSingletonMode(gocron.LimitModeReschedule)` is still written twice.

**Worked proposal** (applied in `scratch/proposal`):

```go
// scheduleBackups runs backups on the configured interval or cron schedule,
// blocking until shutdown; with neither configured it runs a single backup.
func scheduleBackups() error {
	s, err := gocron.NewScheduler()
	if err != nil {
		return errors.Wrap(err, "failed to create scheduler")
	}

	definition, options := backupSchedule()
	if definition == nil {
		execProviderBackups()

		return nil
	}

	job, err = s.NewJob(definition, gocron.NewTask(execProviderBackups), options...)
	if err != nil {
		return errors.Wrap(err, "failed to create job")
	}

	s.Start()
	waitForShutdown(s)

	return nil
}

// backupSchedule returns the job definition and options for the configured
// interval or cron schedule, or a nil definition when backups run once.
func backupSchedule() (gocron.JobDefinition, []gocron.JobOption) {
	options := []gocron.JobOption{gocron.WithSingletonMode(gocron.LimitModeReschedule)}

	if interval := getBackupInterval(); interval != 0 {
		logger.Printf("scheduling to Run every %s", formatIntervalDuration(interval))

		return gocron.DurationJob(time.Duration(interval) * time.Minute),
			append(options, gocron.WithStartAt(gocron.WithStartImmediately()))
	}

	if cron := os.Getenv(envGitBackupCron); cron != "" {
		logger.Printf("scheduling to Run with cron '%s'", cron)

		return gocron.CronJob(cron, false), options
	}

	return nil, nil
}
```

Same number of functions as the head. The difference is which half is the helper. `backupSchedule` is a decision with no scheduler and no blocking, so it can be unit-tested by setting two environment variables. `scheduleBackups` is the only place that touches the scheduler and the global, and it reads top to bottom with one exit per outcome. Interval still takes precedence over cron, the "scheduling to Run" line is still logged after the scheduler is created, and the scheduler is still created in one-shot mode, all as at the head.

**A further step I did not take.** The scheduler is created before the code knows whether it is needed; in one-shot mode it is constructed and never started or shut down. Moving `NewScheduler` below the `definition == nil` check would fix that, but it changes when a scheduler-construction error can surface, so it is a behaviour change and I left it out of a proposal that is otherwise equivalent.

**Verification status.** The proposal builds, vets and passes the package tests in the scratch copy. Neither the head's nor the proposal's interval and cron paths are exercised by any test in the repository (they block on a signal), so equivalence of the scheduling behaviour is by reading. The one-shot path is reached by the live-provider tests, which skip here.

## 3. `validateStartupConfig` (Q1)

**Evidence.** `internal/backup.go:443-471`. The function reads `GIT_BACKUP_DIR` with `os.LookupEnv` (line 444), checks the GitHub orgs/token pairing, trims a trailing newline from the directory (line 456), stats it, runs `checkProvidersDefined`, and returns the trimmed directory.

Elsewhere the same setting is read with `GetEnvOrFile` and not trimmed: `displayStartupConfig` at line 191 and `runProviderBackups` at line 41.

**Observation.** The name says "validate" and the signature says "and also give me the backup directory". The returned value is used only to create the working directory; the backup run resolves the directory again by a different route. With only `GIT_BACKUP_DIR_FILE` set, `displayStartupConfig` prints "root backup directory: …" and then `validateStartupConfig` returns "environment variable GIT_BACKUP_DIR must be set". With a trailing newline in the variable, `Run` creates the working directory under the trimmed path and `runProviderBackups` computes it from the untrimmed one.

All of that predates the PR and the PR preserves it faithfully. I am not counting it as a finding against this change. It is recorded because the new function boundary is the natural place for a single `backupDir() (string, error)` resolver that all three callers use, and which behaviour that resolver should have is a question for the author. It is Q1 in the summary.

**Verification status.** By reading only.
