# Backup orchestration, configuration, and scheduling

## Scope and measurements

This report covers the committed changes to `internal/backup.go`, with
`internal/constants.go`, `internal/envfile.go`, the provider entry points,
and the relevant existing tests as supporting source evidence. Repository
guidance was not loaded as instructions. No external sources were used.

`git diff main...review-head -- internal/backup.go` reports +246/−201.
`git show main:internal/backup.go | wc -l` gives 729 lines; the head has
774 lines. `git show main:internal/backup.go | rg -c '^func '` gives 23
functions; `rg -c '^func ' internal/backup.go` gives 42. Neither measurement
alone establishes a regression. The important question is whether the
additional boundaries reduce repeated behavior or merely move it.

The old `checkProvider` occupies 49 lines, including its declaration and
closing brace. The new caller and its two implementations occupy 64
lines, excluding the intervening comments and blank lines. They continue
to share the same implicit parameter-state model.

## Credential validation

### Finding: model state once and keep errors in the return contract

The new helper signatures at `internal/backup.go:348` and `:369` accept
`outputErrs *strings.Builder` and return only an integer. Error information
travels through writes at lines 358 and 385; `checkProvider` detects those
writes at lines 339–340. The caller must understand that the integer is
only part of each function's result. The helper interface also binds
credential checking to the formatting and ownership of an error buffer.

The token helper scans `enabledProviderAuth[provider]` and interprets
presence and `strings.Trim(val, " ")`. The paired helper duplicates that
interpretation, counts configured parameters in one pass, and then reads
them all again to report missing parameters if only some were configured.
For an Azure DevOps configuration with a username but no PAT, this means
two reads of both credentials. A `_FILE` read is filesystem I/O and can
also log errors; `GetEnvOrFile` is not an in-memory map lookup.

The repeated scans predate the PR. The finding is that the new abstraction
retains those scans and introduces a side-effectful output contract even
though this change explicitly restructures the validator. It is not a
claim that the PR introduced credential misvalidation or an observable
race. A value describing parameter state gives the extraction a useful
boundary and makes the policy easier to assess.

The canonical parameter lists are already in `internal/constants.go:120–147`.
The classification lists at lines 149–159 are disjoint in the reviewed
source. Gitea and Sourcehut each have an API URL and a token in their
parameter list even though they belong to `justTokenProviders`; the
existing implementation can therefore return 2. Do not change these
counts to 0/1 as part of this quality fix. The existing comment claiming
0/1 predates this PR and is not a separate finding.

### Worked proposal

The useful separation is environment inspection followed by credential
policy. This sketch keeps `GetEnvOrFile` as the environment boundary and
uses the existing provider lists. It removes the two validators that
mutate a supplied builder. Error formatting remains local to the
function that returns the error.

```go
type authParameterState struct {
    configured int
    missing    []string
    blank      []string
}

func inspectAuthParameters(params []string) authParameterState {
    var state authParameterState
    for _, param := range params {
        val, exists := GetEnvOrFile(param)
        if exists && strings.Trim(val, " ") != "" {
            state.configured++
            continue
        }
        state.missing = append(state.missing, param)
        if exists {
            state.blank = append(state.blank, param)
        }
    }
    return state
}

func checkProvider(provider string) (int, error) {
    token := slices.Contains(justTokenProviders, provider)
    paired := slices.Contains(userAndPasswordProviders, provider)
    if !token && !paired {
        return 0, nil
    }

    params := enabledProviderAuth[provider]
    state := inspectAuthParameters(params)
    var count int
    var invalid []string
    if token {
        count += state.configured
        invalid = append(invalid, state.blank...)
    }
    if paired {
        if state.configured == len(params) {
            count++
        } else if state.configured > 0 {
            invalid = append(invalid, state.missing...)
        }
    }
    if len(invalid) == 0 {
        return count, nil
    }

    var output strings.Builder
    for _, param := range invalid {
        _, _ = fmt.Fprintf(&output,
            "%s parameter '%s' is not defined.\n", provider, param)
    }
    return count, errors.New(output.String())
}
```

The proposal is intended to preserve validation results for stable
configuration. It deliberately takes one snapshot instead of reading
each credential again while reporting an error. Transient `_FILE`
failures or mid-validation file replacement can change incidental
diagnostics relative to a repeated read; this is a reason to define the
snapshot boundary explicitly, not to promise byte-for-byte equivalence
for changing input. No proposal was applied, compiled, or Sonar-scored.

| Policy | Input | Existing result to preserve |
| --- | --- | --- |
| Token | Parameter absent | No count and no parameter error |
| Token | Parameter exists but is empty or spaces | Parameter error |
| Token | Parameter is nonblank | Count that parameter |
| Token | API URL and token both nonblank | Count both, as currently implemented |
| Paired | No parameter nonblank | No count and no partial-config error |
| Paired | Some parameters nonblank | Count 0; report each missing or blank parameter |
| Paired | Every parameter nonblank | Count 1 |
| Unknown provider | Not in either classification | Count 0, no error |

Error messages must retain the provider label, parameter name, newline,
and parameter-list order. Do not substitute `strings.TrimSpace` for
`strings.Trim(val, " ")`, or promote absent token parameters into errors.
The two Bitbucket methods are handled outside these policies by
`checkProvidersDefined`; keep their existing incomplete-auth behavior.

Verification is static for partial credentials. The selected test run
passes the no-provider case and file precedence case, but those do not
exercise the new mutable helper boundary for partially configured Azure
DevOps credentials. Meaningful follow-up cases should assert the public
`checkProvider` return values and error text for the policy table, rather
than test the internal snapshot's implementation.

## Working-directory resolution

### Finding: create and clean using the same resolver

`createWorkingDir` at `internal/backup.go:473–478` extracts a block from
`Run` but keeps the duplicated environment lookup and fallback join.
`resolveWorkingDir` at lines 140–146 already owns exactly that choice.
`runProviderBackups` uses it at line 52 to choose the directory passed to
the cleanup defer at line 57. Creation now has a named helper but still
does not share the selection policy with cleanup.

This duplication existed inside `Run` before the PR. The actionable
problem is the missed canonical reuse in the newly introduced helper.
The remedy deletes the duplicate policy and leaves filesystem behavior
and messages in the creation function:

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

Both implementations currently treat an empty override as the default,
return a nonempty override verbatim, and use `filepath.Join` for the
default. The creation function already applies `filepath.Clean` when
creating the directory. Equivalence follows directly from those bodies;
no execution of this proposed replacement is claimed.

## Accepted abstractions and behavior tracing

`collectProviderBackupResults` at lines 97–123 keeps the old execution
order: Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, then Sourcehut. Its
local typed table replaces five copies of the same environment gate and
append operation. Azure DevOps still uses the username as the dispatch
gate, with full startup validation elsewhere. This is an ordered
dispatcher, not an unnecessary generic plugin mechanism. The nil result
slice, pointer dereferences, and sequential execution are inherited.
Parallelization would alter provider log interleaving and access to shared
client and working-directory state; it is not an obvious behavior-neutral
remedy here.

The Bitbucket predicates are now reused by execution and validation.
Precomputing API-token completeness before the map iteration makes
deduplication of the two authentication methods independent of map order.
The old loop could count both when OAuth was visited first; that count
only decides whether any provider exists. The new loop still accepts
either complete method, ignores incomplete Bitbucket methods, and runs
Bitbucket once. The execution predicate now short-circuits OAuth reads
when API-token credentials are complete. That can omit incidental
`GetEnvOrFile` diagnostics compared with the old unconditional reads; it
does not establish a material backup behavior regression.

The five startup display functions preserve ordinary gates and log
ordering. The four shared logging helpers each own repeated formatting
and checks, so they earn their indirection. GitLab's historical `Gitlab`
casing for the LFS message is intentionally retained. Bitbucket compare
matching changes from `ToLower(...) == "refs"` to `EqualFold`; ASCII
configuration values behave the same. Unicode fold equivalents can
differ, so absolute byte-for-byte behavior identity is broader than the
source supports. Source line prefixes also move because the logger uses
`log.Lshortfile`. Neither observation merits a blocking finding here.

`Run` now has readable phases for timeout logging, startup validation,
directory creation, and scheduling. Environment gates, backup-directory
newline removal, error wrapping, and the precedence of interval over
cron remain in the same order. `runScheduledJob` owns registration,
startup, and shutdown waiting shared by both scheduled branches; it is
not a pass-through wrapper. Interval jobs still start immediately, cron
jobs do not add that option, and both retain singleton rescheduling.
The global `job`, scheduler creation for one-shot mode, and signal-waiting
implementation all predate this change. They were not reported as new
regressions.

Moving the startup display group or scheduler group into separate files
could make navigation easier, but at 774 lines the file has not crossed
the skill's threshold. No unjustified giant-file finding is supported.

## Executed verification

The single selected package test invocation was run from the clone root.
The cache in the following command is the supplied attempt cache; there
was no dependency download:

```sh
timeout 300s env \
  GOMODCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-029/clone-cache/gomodcache \
  GOCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-029/clone-cache/gocache \
  GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local \
  go test ./internal/ -count=1 -timeout=240s -v \
  -run '^(TestGet.*|TestIsInt|TestGitInstalled|TestGitInstallPathAdditional|TestGitHubEnvs|TestFormatIntervalDuration.*|TestCheckProvidersFailureWhenNoneDefined|TestFailureIfGitBackupDirUndefined|TestAzureDevOpsCredentialsFileSupport|TestGithubRepositoryBackupWithMockAPI|TestWebhook.*)$'
```

Exit status was 0; 20 top-level tests passed, and package execution took
5.140 seconds. The mock GitHub backup cloned and bundled two repositories
using local fixture servers. It calls `githosts.NewGitHubHost` directly and
does not validate the successful `Run` orchestration. No scheduling or
signal test was selected. The invalid-token GitHub test was excluded
because it attempts an external connection even without credentials.

`git diff main...review-head --check` passed. `git status --porcelain=v1`,
`git diff --exit-code`, and `git diff --cached --exit-code` were clean
after execution. Head remained the pinned SHA, and the head tree listing
hash matched the initial measurement. No checkout edits or remedies were
made.
