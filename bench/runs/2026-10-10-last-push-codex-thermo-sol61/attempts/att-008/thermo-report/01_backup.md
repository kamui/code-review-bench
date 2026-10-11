# Backup orchestration, configuration, and credential boundaries

## Evidence and measurements

The reviewed source is `internal/backup.go` at head `136a4850df9aec8cf8813b27ba1533d4a17bc642`. Supporting evidence comes from `internal/constants.go`, `internal/envfile.go`, the provider entry points, and the internal tests. These supporting files were read as source, not as instructions.

`git diff main...review-head -- internal/backup.go` shows 246 additions and 201 deletions. `git show main:internal/backup.go | wc -l` reports 729 lines; `wc -l internal/backup.go` reports 774. Comparing `rg -n '^func ' internal/backup.go` with `git show main:internal/backup.go | rg -n '^func '` gives 42 and 23 top-level functions respectively, including `init`. The new helpers all remain in the original file. This is not a 1,000-line threshold violation, but the expanded function surface makes it especially important that the extractions actually establish coherent ownership.

`nl -ba internal/backup.go` establishes the head anchors. The new provider collector is at lines 95–123, BitBucket predicates at 125–138, startup rendering at 190–294, credential checking at 322–395, and startup/scheduling at 397–541. `nl -ba internal/constants.go` shows the auth map at 120–148 and two membership lists at 149–159. `cat internal/envfile.go` confirms that credential lookup may perform file I/O and emit diagnostics rather than merely read an in-memory value.

## Finding: credential policy still lives in parallel declarations and mutable helper outputs

The changed calls at `internal/backup.go:331–336` determine policy by membership in two separate provider-name slices. Each callee then obtains parameters from a third collection, `enabledProviderAuth`. The independent-parameter helper returns a count and appends diagnostics through a `*strings.Builder`; the all-required helper uses the same side channel and scans its parameter list a second time when configuration is partial. The caller converts the accumulated builder into its returned error. These helpers add an error-accumulation protocol without reducing the number of representations a maintainer must understand.

The issue is retained architectural complexity exposed by the refactor, not a newly demonstrated backup bug. It is actionable in this diff because both classification algorithms and their dispatcher are being restructured here. A provider declaration can already contain its parameter list and policy together; membership slices are unnecessary. A single evaluator can collect populated, missing, and explicitly blank parameters on one pass, apply the declared policy, and return `(int, error)` without sharing a mutable output object with its caller.

The existing policies are not interchangeable. For GitHub and GitLab, a nonblank token counts as one. For Gitea and Sourcehut, both the URL and token are in the independent-parameter map entries: either populated parameter contributes to the count, absent parameters do not produce errors, and explicitly blank parameters do. This means the comment claiming a count of only zero or one is not true for all current entries, but that comment predates this change. Azure DevOps is all-required: none populated is silent, a partial configuration reports each missing parameter, and a complete configuration counts as one. BitBucket's outer validation switch bypasses `checkProvider`, uses exact nonempty checks rather than space trimming, and tolerates incomplete alternatives. A behavior-preserving rewrite must retain these distinctions.

The new `tokenProviders` dispatch table is a useful local deduplication, but its gates describe run eligibility, not startup credential classification. It includes Azure's username and omits Gitea/Sourcehut URL gates. Do not force it to use the validation count as an execution predicate. Naming and data ownership should expose the two concepts rather than imply that every row is a complete token-auth configuration. BitBucket execution remains first, followed by Gitea, GitHub, GitLab, Azure DevOps, and Sourcehut.

## Worked code-judo proposal

Keep the existing outer BitBucket alternatives and replace the auth map plus membership lists with declarations such as the following. The names are illustrative; the contract is explicit and contains no optional policy.

```go
type authPolicy uint8

const (
    independentParameters authPolicy = iota
    allParametersRequired
)

type authSpec struct {
    params []string
    policy authPolicy
}

var providerAuth = map[string]authSpec{
    providerNameGitHub: {
        params: []string{envGitHubToken},
        policy: independentParameters,
    },
    providerNameGitea: {
        params: []string{envGiteaAPIURL, envGiteaToken},
        policy: independentParameters,
    },
    providerNameAzureDevOps: {
        params: []string{envAzureDevOpsUserName, envAzureDevOpsPAT},
        policy: allParametersRequired,
    },
    // Declare the remaining current entries with their current policy.
}
```

The evaluator below replaces the membership dispatch and both builder-mutating helpers. It preserves the order of diagnostics within a provider. Unknown providers must retain the current zero/nil result; changing unsupported-input behavior is a separate contract change.

```go
func checkProvider(provider string) (int, error) {
    spec, known := providerAuth[provider]
    if !known {
        return 0, nil
    }

    var found int
    var missing, invalid []string
    for _, param := range spec.params {
        val, exists := GetEnvOrFile(param)
        if exists && strings.Trim(val, " ") != "" {
            found++
            continue
        }

        message := fmt.Sprintf("%s parameter '%s' is not defined.\n", provider, param)
        missing = append(missing, message)
        if exists && spec.policy == independentParameters {
            invalid = append(invalid, message)
        }
    }

    if spec.policy == allParametersRequired {
        if found == len(spec.params) {
            return 1, nil
        }
        if found == 0 {
            return 0, nil
        }
        found, invalid = 0, missing
    }
    if len(invalid) > 0 {
        return found, errors.New(strings.Join(invalid, ""))
    }
    return found, nil
}
```

This removes two membership collections, two helper boundaries, the mutable builder parameter, and the partial-configuration second lookup loop. It does not require a plugin registry, a generic callback framework, or concurrent backups. The typed declarations should live in a focused configuration file if the author decomposes the module; moving the same collections unchanged would only redistribute the complexity.

The separate scratch file `../auth_policy_proof_test.go` implements the current and proposed classifications and checks all lists of zero through three parameters over seven states: absent, explicitly empty, space-only, populated, populated with surrounding spaces, tab-only, and newline-only. Both policies are tested, giving `2 * (1 + 7 + 49 + 343) = 800` combinations. Counts and ordered invalid-parameter indexes match. The empty-list case preserves the current all-required evaluator's one-count behavior even though current declarations are nonempty. Tabs and newlines intentionally remain nonblank because the existing code uses `strings.Trim(val, " ")`, not `TrimSpace`.

The command, run from the clone root, was:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-008/clone-cache/gomodcache \
GOCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-008/clone-cache/gocache \
GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local \
go test /home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-008/clone-work/auth_policy_proof_test.go -count=1 -v -timeout=4m
```

It passed and reported `verified 800 policy/state combinations`; package execution took 0.003 seconds. This is classification verification for stable input observations. It is not a compiled integration of the illustrative repository snippet. It does not exercise `GetEnvOrFile`, exact error string serialization, lookup diagnostics, concurrent secret rotation, or the BitBucket alternative selection. Reducing repeated file reads changes incidental lookup diagnostics, so characterize those separately if they are a supported logging contract. The current intra-provider classification is the behavior this proof establishes.

## Finding: working-directory creation bypasses the canonical resolver

The changed `createWorkingDir` at lines 473–487 repeats the four-line path-choice policy at 475–478. `resolveWorkingDir` already owns exactly that override-and-default logic at 140–146, and `runProviderBackups` calls it at line 52 before deferring cleanup of that path. Startup creation and backup cleanup should use the same selection function. This is a missed canonical-helper reuse opportunity in the extraction, not evidence of an existing mismatch for today's inputs.

The remedy is exact and local:

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

Both implementations read `os.Getenv(envGitWorkingDir)` and otherwise use `filepath.Join(backupDIR, workingDIRName)`. The replacement therefore preserves the selected path and leaves creation, permissions, cleaning, and errors in their current owner. Verification is source equivalence; no production change was applied.

## Behavior review and verification limits

The provider collector retains provider order and each provider's prior gate. The BitBucket API predicate now short-circuits the OAuth predicate when API credentials are complete. This can suppress diagnostics from unread OAuth credential files, so the PR's blanket statement that all log output is unchanged is broader than source evidence supports. This is an incidental lookup/log difference rather than a demonstrated operational defect, and it is not a separate actionable finding. Precomputing the BitBucket API-complete flag also avoids the previous map-iteration-dependent count of both auth alternatives; callers only test whether the count is zero, so the externally used result is unchanged for stable credentials.

Startup logging keeps existing labels, including `Organistations` and the `Gitlab` LFS spelling, and the same per-provider ordering. The shared compare helper retains the clone default and case-insensitive refs choice. `logProviderBackupLFS` preserves the existing combination of a `GetEnvOrFile` existence check and `envTrue`'s direct environment lookup; this refactor does not fix the preexisting boolean `_FILE` inconsistency. The logger uses `log.Lshortfile`, so extracting and moving logging sites necessarily changes their source prefixes even where the message bodies match.

`Run` retains git validation, startup logging, version probing, timeout validation, directory validation, provider validation, directory creation, and scheduling in the same order. The new scheduler helper preserves interval precedence over cron, interval start-immediately behavior, singleton rescheduling, assignment to the existing global job, start, and shutdown wait. It earns its abstraction by deleting duplicated registration/start/wait sequences. The existing unconditional scheduler creation in one-shot mode and global job state were not introduced here; neither is presented as a new defect. There is no justification for parallelizing provider backups merely to satisfy an orchestration heuristic: shared globals and filesystem operations require a separate concurrency design.

The exact offline internal command was:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-008/clone-cache/gomodcache \
GOCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-008/clone-cache/gocache \
GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local \
go test ./internal/ -count=1 -timeout=4m
```

It passed with `ok github.com/jonhadfield/soba/internal 5.241s`. This compiles the changed internal code and exercises the available suite, including local fixtures. Credentials were unavailable and live tests skip. The existing no-provider test checks an important validation failure, but no dedicated credential-policy matrix, scheduler shutdown test, startup log comparison, or collector dispatch test exists in the read test sources. The local GitHub fixture exercises the dependency's host directly, not `Run` or the new collector. No claim is made that the suite proves complete behavior preservation.

The review did not run build, vet, golangci-lint, or Docker, and does not repeat the PR author's verification claims as independent evidence. All proposals remain unapplied. `git diff main...review-head --check` and final checkout status were clean.
