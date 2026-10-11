# 02 — `Run` decomposition and scheduling

Scope: `internal/backup.go` lines 397–541 on the head `136a4850` (`Run`, `logRequestTimeout`,
`validateStartupConfig`, `createWorkingDir`, `scheduleBackups`, `runScheduledJob`) and the
pre-existing `resolveWorkingDir` at lines 140–146. Covers summary findings F3, F4 and F5.

## What the PR did

`Run` on the base was one ~110-line function. On the head it is 25 lines that call five new
helpers in sequence. The statements inside the helpers are the base's statements in the base's
order; I compared them hunk by hunk in `git diff main...review-head -- internal/backup.go` and
found no reordering and no changed condition other than the mechanical merge of
`if ghOrgsExists { if !githubTokenExists {` into one `&&` (`backup.go:452`).

Baseline health: `go vet ./internal/` is clean and `go test ./internal/ -count=1` reports
`ok github.com/jonhadfield/soba/internal 5.241s` on the head under the offline environment. The
live provider tests skip for want of credentials, so the scheduling path and the provider run
path are not exercised by that result.

## F3 — `createWorkingDir` re-implements `resolveWorkingDir`

### Evidence

```go
// backup.go:140-146, unchanged by the PR
func resolveWorkingDir(backupDir string) string {
	if w := os.Getenv(envGitWorkingDir); w != "" {
		return w
	}

	return filepath.Join(backupDir, workingDIRName)
}
```

```go
// backup.go:473-478, extracted by the PR
func createWorkingDir(backupDIR string) error {
	// Check if GIT_WORKING_DIR is set, otherwise use default
	workingDIR := os.Getenv(envGitWorkingDir)
	if workingDIR == "" {
		workingDIR = filepath.Join(backupDIR, workingDIRName)
	}
```

The two are the same rule. `resolveWorkingDir` is what `runProviderBackups` uses at
`backup.go:52` to decide which directory to clean up; `createWorkingDir` decides which directory to
create. If one is changed without the other, soba creates one directory and removes a different
one.

### Remedy

```go
func createWorkingDir(backupDIR string) error {
	workingDIR := resolveWorkingDir(backupDIR)

	logger.Println("creating working directory:", workingDIR)
	...
```

Status: **verified by reading**; the two bodies are equivalent expression for expression, so no
execution was needed.

## F4 — Helper boundaries follow line ranges rather than concepts

### Evidence

`validateStartupConfig() (string, error)` (`backup.go:443-471`) does four unrelated things: it
resolves and normalises the backup directory, enforces the GitHub orgs-need-a-token rule, stats the
directory, and validates provider credentials. Its return value exists only so `Run` can pass it to
`createWorkingDir` on the next line (`backup.go:412-417`). A function named "validate" that is also
the only producer of the resolved backup directory is a contract a reader has to open to
understand.

`logRequestTimeout() error` (`backup.go:426-439`) is where an invalid `GIT_REQUEST_TIMEOUT` aborts
startup. A reader scanning `Run` for the places configuration can be rejected will look at
`validateStartupConfig` and miss it.

The backup directory is resolved three times per start-up, by two mechanisms:

| Site | Mechanism |
| --- | --- |
| `displayStartupConfig`, `backup.go:191` | `GetEnvOrFile(envGitBackupDir)` — honours `GIT_BACKUP_DIR_FILE` |
| `validateStartupConfig`, `backup.go:444` | `os.LookupEnv(envGitBackupDir)`, then `strings.TrimSuffix(…, "\n")` |
| `runProviderBackups`, `backup.go:41` | `GetEnvOrFile(envGitBackupDir)` — no newline trim |

The inconsistency predates the PR. The extraction was the opportunity to make one function own
"what is the backup directory" and instead it drew a helper boundary around the odd one out. Read
from the code; not executed.

### Worked proposal

Split by concept, so that `Run` reads as resolve → validate → prepare → schedule and each step's
name says what can fail in it:

```go
func Run() error {
	...
	backupDIR, err := resolveBackupDir() // lookup, trim, stat
	if err != nil {
		return err
	}

	if err = validateConfig(); err != nil { // request timeout, GitHub orgs/token, providers
		return err
	}

	if err = createWorkingDir(backupDIR); err != nil {
		return err
	}

	return scheduleBackups()
}
```

This reorders two checks relative to today (the GitHub orgs rule would run after the directory
stat rather than before; the request-timeout check after rather than before), which only changes
which error is reported first when several are wrong at once. If strict order must be preserved,
keep the request-timeout call first and move only the orgs rule. Status: **proposal, not
compiled.**

## F5 — Scheduling: hidden global write and a helper that owns half the flow

### Evidence

`runScheduledJob` (`backup.go:525-541`) assigns the package-level `job` (`backup.go:737`) through a
`var err error` shim that exists only because `job, err :=` would shadow the global:

```go
func runScheduledJob(s gocron.Scheduler, definition gocron.JobDefinition, options ...gocron.JobOption) error {
	var err error

	job, err = s.NewJob(
```

`job != nil` is how the rest of the file tells one-shot mode from scheduled mode: it decides
whether a failed run calls `os.Exit(1)` (`backup.go:35`) and whether the next-run banner is logged
(`backup.go:87`). On the base that assignment sat in `Run`, in the same `switch` that chose the
mode. On the head it is two calls down, in a helper whose signature gives no hint that it sets the
process's mode flag.

`scheduleBackups` (`backup.go:491-521`) builds a `gocron.Scheduler` before it knows whether one is
needed, so the one-shot `default` arm (`backup.go:516-517`) leaves an unused scheduler behind. Two
arms `return runScheduledJob(...)` while the third falls out of the `switch` to a trailing
`return nil`. `gocron.WithSingletonMode(gocron.LimitModeReschedule)` is written in both arms
(`backup.go:506,514`).

### Worked code-judo proposal

The `switch` only chooses *data* — a job definition and its options. Let it return that data and
there is one place that creates the scheduler, registers the job, sets the mode flag and blocks:

```go
// backupSchedule returns the configured job definition and options, or a nil
// definition when backups should run once.
func backupSchedule() (gocron.JobDefinition, []gocron.JobOption) {
	singleton := gocron.WithSingletonMode(gocron.LimitModeReschedule)

	if interval := getBackupInterval(); interval != 0 {
		logger.Printf("scheduling to Run every %s", formatIntervalDuration(interval))

		return gocron.DurationJob(time.Duration(interval) * time.Minute),
			[]gocron.JobOption{singleton, gocron.WithStartAt(gocron.WithStartImmediately())}
	}

	if cron := os.Getenv(envGitBackupCron); cron != "" {
		logger.Printf("scheduling to Run with cron '%s'", cron)

		return gocron.CronJob(cron, false), []gocron.JobOption{singleton}
	}

	return nil, nil
}

func scheduleBackups() error {
	definition, options := backupSchedule()
	if definition == nil {
		execProviderBackups()

		return nil
	}

	s, err := gocron.NewScheduler()
	if err != nil {
		return errors.Wrap(err, "failed to create scheduler")
	}

	job, err = s.NewJob(definition, gocron.NewTask(execProviderBackups), options...)
	if err != nil {
		return errors.Wrap(err, "failed to create job")
	}

	s.Start()
	waitForShutdown(s)

	return nil
}
```

`runScheduledJob` disappears, the `var err error` shim disappears, the write to `job` sits beside
the branch that makes it meaningful, and `backupSchedule` is a pure-ish function a unit test can
call without starting a scheduler or blocking on a signal.

### Verification

`backupSchedule` as written above was compiled and exercised in the scratch copy
(`clone-work/scratch/soba/internal/zz_probe_test.go`, `TestProbeScheduleShape`) against the pinned
gocron version:

```
one-shot: definition nil=true options=0
interval: definition nil=false options=2
cron: definition nil=false options=1
```

Status: **`backupSchedule` verified by execution; the rewritten `scheduleBackups` tail is not
executed** (it blocks on a signal). One intended difference from the head: the one-shot path no
longer constructs a scheduler it never starts. If scheduler construction can fail in an
environment where one-shot mode currently fails too, that failure would go away; I do not regard
that as behaviour worth preserving, but it is a difference.
