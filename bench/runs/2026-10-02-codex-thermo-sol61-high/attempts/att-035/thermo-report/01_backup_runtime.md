# Backup orchestration and configuration

## Scope and measurements

Compared the full base and head versions of `internal/backup.go`, its diff,
`internal/constants.go`, `internal/envfile.go`, all relevant test callers, and
the Bitbucket, Gitea, Sourcehut, and Azure DevOps adapters. Provider knowledge
was traced through the declarations and call sites, rather than inferred from
helper names or the PR description.

Commands used for the source evidence included:

```sh
git diff main...review-head -- internal/backup.go
nl -ba internal/backup.go
nl -ba internal/constants.go
nl -ba internal/envfile.go
rg -n 'checkProvider|enabledProviderAuth|justTokenProviders|userAndPasswordProviders|resolveWorkingDir|createWorkingDir|scheduleBackups' internal main.go
git show main:internal/backup.go
```

Line counts and function counts were measured with Python by reading each head
file, reading its base blob with `git show main:<path>`, counting `splitlines()`,
and matching declarations with `^func `. Function spans were measured by finding
declarations and their closing braces; these are source-span measurements, not
a claim to have run a cognitive-complexity analyzer.

| Function | Base span | Head span |
| --- | ---: | ---: |
| `runProviderBackups` | 88 | 54 |
| `displayStartupConfig` | 110 | 11 |
| `checkProvider` | 49 | 19 |
| `Run` | 117 | 26 |
| `checkProvidersDefined` | 48 | 38 |

`backup.go` grows from 729 to 774 lines and from 23 to 42 functions. The new
collection helper is 27 lines. The two extracted validation helpers are 18 and
27 lines, so that validation path is now 64 function-body/span lines across
three functions, compared with 49 in one function before. That observation is
not by itself a defect: focused helpers can justify additional lines. Here it
identifies where the larger policy model is still missing.

## Finding: consolidate provider policy

The actionable finding is anchored to the new execution table at
`internal/backup.go:105–114`. Supporting evidence is the dispatch at lines
331–336, validation helpers at 348–395, aggregation at 680–717, and declarations
in `internal/constants.go:120–159`.

The collection loop is a useful improvement over five manually repeated `if`
blocks. It maintains their order: Gitea, GitHub, GitLab, Azure DevOps, then
Sourcehut, with Bitbucket executed before the loop. However, it represents just
one piece of a provider's policy. The auth parameter map and two provider-name
classification lists remain the other pieces. Matching a string in a map does
not ensure membership in either classification list, and matching a
classification list does not ensure an execution entry exists. Readers must
join these structures mentally.

The new validation helpers preserve the misleading common integer contract.
`checkJustTokenProvider` increments for every nonblank parameter, including
API URLs for Gitea and Sourcehut. It may return 2, although `checkProvider`'s
comment promises 0 or 1. `checkUserAndPasswordProvider` instead returns 1 only
for a complete tuple. The aggregation uses only whether the final sum is zero,
so the count is incidental bookkeeping. The comment mismatch and policy split
already existed before this PR; they are evidence of the missed simplification,
not newly introduced runtime bugs.

The mutable builder argument makes a helper's result incomplete without
observing a side effect in its caller. There are no independent callers of
these new validation helpers. This is a good place to return an explicit
configured boolean and an error, and to express the policy in the descriptor
instead of reconstructing it from a provider string.

### Worked code-judo proposal

Use a small package-local model, colocated with provider configuration. This
is a concrete design sketch, not an applied or compiled patch:

```go
type validationPolicy uint8

const (
    independentParameters validationPolicy = iota
    completeTuple
)

type providerSpec struct {
    name       string
    gate       string
    parameters []string
    policy     validationPolicy
    backup     func(string) *ProviderBackupResults
}

var ordinaryProviders = []providerSpec{
    {providerNameGitea, envGiteaToken,
        []string{envGiteaAPIURL, envGiteaToken}, independentParameters, Gitea},
    {providerNameGitHub, envGitHubToken,
        []string{envGitHubToken}, independentParameters, GitHub},
    {providerNameGitLab, envGitLabToken,
        []string{envGitLabToken}, independentParameters, Gitlab},
    {providerNameAzureDevOps, envAzureDevOpsUserName,
        []string{envAzureDevOpsUserName, envAzureDevOpsPAT}, completeTuple, AzureDevOps},
    {providerNameSourcehut, envSourcehutToken,
        []string{envSourcehutAPIURL, envSourcehutToken}, independentParameters, Sourcehut},
}
```

The execution gate is deliberately separate from startup validation. Today an
API URL alone can count toward startup validation for Gitea or Sourcehut, but
only a token starts a backup for either. A complete validation tuple is required
for Azure DevOps startup, while execution itself gates on the username. A
behavior-preserving refactor must represent these distinctions rather than
replace them all with a single `credentialsComplete` predicate.

Validation can collect evidence in one pass and return it directly:

```go
func validateProvider(p providerSpec) (bool, error) {
    var present int
    var missing, blankDefined []string
    for _, parameter := range p.parameters {
        value, exists := GetEnvOrFile(parameter)
        if exists && strings.Trim(value, " ") != "" {
            present++
            continue
        }
        message := fmt.Sprintf("%s parameter '%s' is not defined.\n",
            p.name, parameter)
        missing = append(missing, message)
        if exists {
            blankDefined = append(blankDefined, message)
        }
    }

    switch p.policy {
    case independentParameters:
        if len(blankDefined) != 0 {
            return present > 0, errors.New(strings.Join(blankDefined, ""))
        }
        return present > 0, nil
    case completeTuple:
        if present > 0 && len(missing) > 0 {
            return false, errors.New(strings.Join(missing, ""))
        }
        return len(missing) == 0, nil
    default:
        panic("invalid provider validation policy")
    }
}
```

Both aggregate validation and collection iterate `ordinaryProviders`. Collection
still reads each gate immediately before that provider's backup, preserving
the existing sequential execution semantics. Aggregate validation accumulates
`configured = configured || providerConfigured` and appends returned errors,
then handles the no-provider case exactly as today. Bitbucket contributes an
explicit `bitbucketAPITokenDefined() || bitbucketOAuthDefined()` configuration
check and its existing execution branch. No fake Bitbucket token gate, callback
registry, reflection, concurrency, or global credential snapshot is needed.

This design deletes `justTokenProviders`, `userAndPasswordProviders`, the
string-based `checkProvider` dispatcher, and its two builder-mutating helpers.
It replaces the ordinary-provider rows in `enabledProviderAuth` rather than
adding a fifth source of metadata. The obsolete Bitbucket rows can disappear
once their remaining references are removed; its two authentication methods
remain explicit in the Bitbucket predicates and adapter. The missing/blank
rules, error message text, and provider execution order remain visible.

There are two behavior details to characterize before implementing the sketch.
First, map iteration currently leaves cross-provider error order unspecified;
an ordered descriptor would make it stable. Second, the complete-tuple helper
currently rereads parameters to build partial-configuration errors. A one-pass
validator observes one read per parameter instead of two, so changing secret
files during validation can affect diagnostics differently. Neither requires
retaining the parallel registries; do not claim byte-for-byte diagnostic
equivalence for moving file contents without explicitly testing or specifying
that boundary.

Verification status: the structural evidence is confirmed by source inspection.
The proposal was not compiled or executed, and no checkout edits were made.
Tests should cover unset, explicitly blank, spaces-only, partial, and complete
parameters; `_FILE` precedence; API-URL-only Gitea/Sourcehut configurations;
Bitbucket API token, OAuth, both methods, and partial methods; and ordinary
provider execution order using local doubles. Avoid converting a
behavior-preserving cleanup into a stricter credential-validation change.

## Finding: reuse working-directory resolution

`createWorkingDir` at lines 473–487 contains an exact repetition of the policy
already in `resolveWorkingDir` at 140–146: a nonempty `GIT_WORKING_DIR` wins,
otherwise use `filepath.Join(backupDir, workingDIRName)`. Execution calls the
canonical helper at line 52, then sends its result to deferred cleanup.

The duplicate existed inside `Run` before the PR. Extracting it into a new
named helper is an opportunity to eliminate it, and the selected review
standard explicitly asks for canonical-helper reuse. This is a smaller
finding than the provider-policy model. The actual outputs agree today.

The complete replacement is direct:

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

For an unset variable, an empty variable, an explicit relative directory, and
an explicit absolute directory, both current copies return the same string.
The proposed replacement keeps the same lookup time, cleaning operation,
permissions, log message, and error context. These are source-equivalence
checks; the proposal was not run and no extra tests were added.

## Other runtime paths examined

The startup logging extraction preserves the order of providers and of their
messages. Common organization, retention, comparison, and LFS operations now
have reusable helpers. The provider-specific wrappers retain meaningful
differences: GitHub's skip-user message, GitLab's minimum access level, and the
existing Bitbucket email gate. A generic optional-field/callback logging
framework would introduce more concepts to preserve this order, so it is not
required by this review. Existing capitalization, organization spelling, and
the environment-only `envTrue` behavior were not escalated as new defects.

`Run` retains the original sequence: find Git, display startup settings, log Git
version, parse/log request timeout, validate directory and providers, create
working directory, then select scheduling mode. The GitHub organization/token
presence check, newline trimming, directory errors, and provider errors remain
equivalent under stable inputs.

`runScheduledJob` removes genuinely repeated registration/start/shutdown code.
Both schedule modes still use singleton rescheduling; only interval mode starts
immediately. Interval still takes precedence over cron. The global `job`
assignment and error wrapping have the same semantics. The helper is not an
identity wrapper: it owns a coherent scheduler lifecycle.

The scheduler is still created for one-shot runs, and signal registration is
still not stopped afterward. These behaviors predate the diff and were not
promoted to findings. Introducing concurrent provider backups would interact
with shared state, working-directory cleanup, and provider ordering; no obvious
safe parallelization follows from this PR, so no concurrency change is required.

Bitbucket completeness is now computed before validation's map iteration. This
removes dependence of its count on whether OAuth is visited before API-token
configuration; only zero/nonzero affects the returned startup result. Collection
now short-circuits OAuth credential reads if API-token credentials are complete.
The chosen provider and preferred authentication method remain the same for
stable files, but missing OAuth-file diagnostics can be suppressed at that
collection stage. Provider adapters still reread credentials. This is an
incidental diagnostic difference, not a confirmed backup correctness regression.

## Verification boundaries

The internal package test command and its outcome are documented in
[02_backup_tests.md](02_backup_tests.md). Source inspection confirms equivalence
of the principal branches; it does not establish behavior of scheduled jobs on
signals or live credentialed backups. No additional scheduler, network, or
scratch-module execution was performed. No file-size threshold was crossed.
