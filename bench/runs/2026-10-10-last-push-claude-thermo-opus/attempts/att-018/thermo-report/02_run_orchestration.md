# 02 — `Run` orchestration and scheduling

Covers summary findings F3 and F4. Line numbers are on the head (`136a485`).

## What the PR did

Base `Run` was a single function of about 115 lines (base `backup.go:370-486`). Head `Run` (`:397-422`) is 25 lines that call, in order, `logRequestTimeout` (`:426-439`), `validateStartupConfig` (`:443-471`), `createWorkingDir` (`:473-487`) and `scheduleBackups` (`:491-521`), the last of which delegates to `runScheduledJob` (`:525-541`). Statement order is preserved; I compared the base body against the concatenated head helpers and found no reordering. The `ghOrgsExists` nested `if` was flattened into `ghOrgsExists && !githubTokenExists` (`:452`), which is equivalent.

Verification status: by reading only. `Run` needs a real backup directory, the scheduler and (for the non-default branches) a signal to return; the credential-free tests that call it (`TestFailureIfGitBackupDirUndefined`, `backup_test.go:804-820`) reach only the first error return of `validateStartupConfig` and pass on the head.

## F3 — `createWorkingDir` duplicates `resolveWorkingDir`

### Evidence

Existing helper, untouched by the PR (`backup.go:140-146`):

```go
func resolveWorkingDir(backupDir string) string {
	if w := os.Getenv(envGitWorkingDir); w != "" {
		return w
	}

	return filepath.Join(backupDir, workingDIRName)
}
```

New function (`backup.go:473-487`):

```go
func createWorkingDir(backupDIR string) error {
	// Check if GIT_WORKING_DIR is set, otherwise use default
	workingDIR := os.Getenv(envGitWorkingDir)
	if workingDIR == "" {
		workingDIR = filepath.Join(backupDIR, workingDIRName)
	}

	logger.Println("creating working directory:", workingDIR)
	...
```

Same variable, same fallback, same join. `resolveWorkingDir` is what `runProviderBackups` uses to compute the directory it later hands to `cleanupWorkingDir` (`:52-57`), so the create path and the cleanup path each carry their own copy of the rule.

### Remedy

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

Behaviour-identical by inspection. It compiles and the package tests pass with it on the scratch copy described in `01_provider_configuration.md` ("Proposal check"); no credential-free test reaches the `MkdirAll` itself.

### The sibling helpers

`logRequestTimeout() error` (`:426-439`). The name says it logs; the signature says it can abort start-up. It is the only place an invalid `GIT_REQUEST_TIMEOUT` is rejected (`getHTTPClient` at `:574` discards the error). A function whose job is to validate a setting and report it belongs with the other validation, or should at least be named for it.

`validateStartupConfig() (string, error)` (`:443-471`). It validates, and it also resolves and returns the backup directory, using `os.LookupEnv(envGitBackupDir)` and `strings.TrimSuffix(backupDIR, "\n")`. The same setting is read two other ways in the same file:

| Site | Reader |
| --- | --- |
| `:41` `runProviderBackups` | `GetEnvOrFile(envGitBackupDir)` — honours `GIT_BACKUP_DIR_FILE`, no newline trim |
| `:191` `displayStartupConfig` | `GetEnvOrFile(envGitBackupDir)` |
| `:444-456` `validateStartupConfig` | `os.LookupEnv` + `TrimSuffix("\n")` — ignores `_FILE` |

All three predate the PR. What the PR adds is a return value: the trimmed directory leaves `validateStartupConfig`, is passed to `createWorkingDir`, and is then discarded; `scheduleBackups` takes no arguments and the run re-reads the variable through the other reader. So the working directory is created under one resolution of the path and cleaned up under another. A single `backupDir() (string, bool)` accessor used by all three sites would let `validateStartupConfig` go back to returning only `error` and would remove the hand-off. Because that changes behaviour for `GIT_BACKUP_DIR_FILE` and for values with a trailing newline, it is a follow-up rather than something this PR should have slipped in.

## F4 — `scheduleBackups` and `runScheduledJob`

### Evidence

`backup.go:491-541`. Points of friction:

- Two `switch` arms end in `return runScheduledJob(...)`; the `default` arm falls through to a trailing `return nil` (`:500-520`).
- `gocron.WithSingletonMode(gocron.LimitModeReschedule)` is written in both arms (`:506, :514`).
- `runScheduledJob` opens with `var err error` (`:526`) for the sole purpose of writing `job, err = s.NewJob(...)` into the package-level `job` (`:737`), which `execProviderBackups` then reads as its one-shot/scheduled mode flag (`:35`).
- The helper's name says "run"; it registers, starts and blocks on a signal.

### Worked proposal (statement order preserved; compiled and unit-tested on the scratch copy, scheduled arms not exercised)

```go
func scheduleBackups() error {
	backupInterval := getBackupInterval()
	backupCron := os.Getenv(envGitBackupCron)

	s, err := gocron.NewScheduler()
	if err != nil {
		return errors.Wrap(err, "failed to create scheduler")
	}

	var definition gocron.JobDefinition

	options := []gocron.JobOption{gocron.WithSingletonMode(gocron.LimitModeReschedule)}

	switch {
	case backupInterval != 0:
		logger.Printf("scheduling to Run every %s", formatIntervalDuration(backupInterval))

		definition = gocron.DurationJob(time.Duration(backupInterval) * time.Minute)
		options = append(options, gocron.WithStartAt(gocron.WithStartImmediately()))
	case backupCron != "":
		logger.Printf("scheduling to Run with cron '%s'", backupCron)

		definition = gocron.CronJob(backupCron, false)
	default:
		execProviderBackups()

		return nil
	}

	if job, err = s.NewJob(definition, gocron.NewTask(execProviderBackups), options...); err != nil {
		return errors.Wrap(err, "failed to create job")
	}

	s.Start()
	waitForShutdown(s)

	return nil
}
```

One function, one `switch` with three straight-line arms and one `if`; the helper, its variadic parameter, the `var err error` and the repeated option are gone. The option order handed to `NewJob` is unchanged (singleton mode first, then start-at for the interval case).

A further step, which does change behaviour slightly and so is optional: move the `default` arm above `gocron.NewScheduler()`, so one-shot runs do not construct a scheduler they never start or shut down.
