# Provider selection and credential validation

## Finding: Consolidate credential validation around explicit policy and one scan

In `internal/backup.go:331–336`, the new `checkJustTokenProvider` and `checkUserAndPasswordProvider` calls split the old validator without simplifying its model: the caller still classifies a provider through two separate global lists, both helpers look its parameters up again in `enabledProviderAuth`, and both mutate the caller's error builder. The grouped-credential helper also reads every parameter twice when configuration is partial. This leaves provider policy spread across three declarations and three functions, with the names obscuring that Gitea and Sourcehut each have two parameters in the supposedly token-only group. Replace the parallel classifications with an explicit validation policy alongside each parameter list, scan each parameter once to collect valid and missing states, and return the count and diagnostics from that boundary. Preserve the current distinction between errors for defined-but-blank independent parameters and errors for partially configured credential groups. This is a missed structural simplification in the touched validator, not an identified new runtime failure.

## Source evidence

The review read `git diff main...review-head -- internal/backup.go`, the full head file, `git show main:internal/backup.go`, `internal/constants.go`, and `internal/envfile.go`. Line references below address the pinned head.

`checkProvider` at lines 326–344 retains two `slices.Contains` checks against independently maintained classification lists. The extracted helpers receive only a provider string and a mutable `*strings.Builder`; they recover the parameter list from the global map. To understand a provider's validation, a reader must resolve its map entry, determine which classification lists contain it, follow the two possible dispatch branches, and account for the builder side effect separately from the integer return.

The map at `internal/constants.go:119–148` describes parameter lists. `justTokenProviders` at lines 149–154 and `userAndPasswordProviders` at lines 155–159 describe policy elsewhere. Gitea's list contains API URL and token; Sourcehut's contains API URL and token. Consequently, "token-only" does not mean a single token parameter or a requirement that all parameters be set. The extracted name encodes an approximate historical grouping rather than the actual validation rule.

`checkJustTokenProvider` at `backup.go:348–365` skips absent parameters, reports defined parameters containing only ASCII spaces, and counts each other parameter separately. It can return two for Gitea or Sourcehut, despite `checkProvider`'s pre-existing comment describing 0 or 1. This is existing behavior to retain, not a newly introduced counting bug.

`checkUserAndPasswordProvider` at lines 369–395 first counts valid parameters. When at least one but fewer than all are valid, its second loop calls `GetEnvOrFile` on every parameter again to produce missing-parameter diagnostics. For Azure DevOps's two parameters this is four reads for a partial configuration, rather than two. The reader in `envfile.go` can open and read a file and log failures; this is not merely a pure lookup. There is no reason to read the missing state again when the first scan already determined it. No concurrent-file-change failure was reproduced; the finding concerns ownership and avoidable repeated work.

The two helpers add boundaries while preserving the full old model. This review treats that as a missed simplification in a refactor explicitly aimed at complexity, rather than labeling the pre-existing map/list design as a new defect.

## Behavior constraints

| Current policy | Parameter state | Count and diagnostics |
| --- | --- | --- |
| Independent parameters | All absent | Zero; no errors |
| Independent parameters | Defined blank or ASCII spaces | Zero for that parameter; its diagnostic is emitted |
| Independent parameters | One valid, another absent | One; no error for the absent parameter |
| Independent parameters | Two valid | Two; no errors |
| Complete group | All absent or blank | Zero; no errors |
| Complete group | Some valid, others absent or blank | Zero; diagnostics for every missing parameter |
| Complete group | All valid | One; no errors |

The independent rule describes GitHub, GitLab, Gitea, and Sourcehut. The complete-group rule is relevant to Azure DevOps in `checkProvidersDefined`. Bitbucket entries are intercepted by that function and evaluated through `bitbucketAPITokenDefined` and `bitbucketOAuthDefined`, which use raw non-empty checks, not ASCII-space trimming. Preserve that difference while simplifying this validator.

Preserve the parameter slice order when formatting diagnostics. The existing outer map iteration does not define a stable order among providers; this remedy need not introduce a new ordering guarantee. Keep `strings.Trim(value, " ")` unless whitespace handling is intentionally changed in a separate behavior change. Preserve environment precedence and file trimming by continuing to call `GetEnvOrFile`.

## Worked code-judo proposal

Move validation policy next to the parameter list, using a typed policy with the two actual rules. Replace the parameter map and the two classification slices together, rather than adding a fourth declaration. The following excerpt illustrates a replacement entry shape and the complete scanner. It is a proposal, not an applied patch.

```go
type authPolicy uint8

const (
    independentParams authPolicy = iota
    completeGroup
)

type providerAuthSpec struct {
    params []string
    policy authPolicy
}

// Migrate every existing entry; these illustrate the two relevant rules.
var providerAuth = map[string]providerAuthSpec{
    providerNameGitea: {
        params: []string{envGiteaAPIURL, envGiteaToken},
        policy: independentParams,
    },
    providerNameAzureDevOps: {
        params: []string{envAzureDevOpsUserName, envAzureDevOpsPAT},
        policy: completeGroup,
    },
}

func checkProvider(provider string) (int, error) {
    spec, known := providerAuth[provider]
    if !known {
        return 0, nil // Retains the old unknown-provider result.
    }

    valid := 0
    missing := make([]string, 0, len(spec.params))
    for _, param := range spec.params {
        value, exists := GetEnvOrFile(param)
        if exists && strings.Trim(value, " ") != "" {
            valid++
            continue
        }
        if spec.policy == completeGroup || exists {
            missing = append(missing, param)
        }
    }

    count := valid
    if spec.policy == completeGroup {
        if valid == len(spec.params) {
            return 1, nil
        }
        if valid == 0 {
            return 0, nil
        }
        count = 0
    }

    if len(missing) == 0 {
        return count, nil
    }
    var diagnostics strings.Builder
    for _, param := range missing {
        fmt.Fprintf(&diagnostics,
            "%s parameter '%s' is not defined.\n", provider, param)
    }
    return count, errors.New(diagnostics.String())
}
```

The full migration must include all seven existing entries. Assign independent policy to the four current independent providers and complete-group policy to the three current grouped entries. Keep the special Bitbucket handling in `checkProvidersDefined` and update its iteration to the new map. The generic count semantics even retain the old all-valid empty-group result, though no actual configured group is empty. Removing the classification slices removes a place for policy drift, and removing the builder parameter makes diagnostics owned by the validator that returns them.

This proposal deletes both extracted scanner helpers and both membership checks. It replaces the second environment-reading pass with a pass over already collected parameter names. It does not unify startup display gates with run-selection gates, since those are not currently equivalent. It does not cache credentials across scheduled runs.

## Selection and Bitbucket changes without separate findings

`collectProviderBackupResults` at lines 97–123 retains Bitbucket, Gitea, GitHub, GitLab, Azure DevOps, Sourcehut execution order and the former checks for the five latter providers. Its table's `envVar` field is a selection key, not a complete authentication specification: Azure DevOps uses a username key there. This distinction limits any proposal to reuse validation policy for dispatch; making dispatch require complete groups would change direct `runProviderBackups` behavior for malformed configurations.

The extracted Bitbucket checks are shared by dispatch and startup validation, which removes real duplicate credential logic. Precomputing API-token completeness before iterating the map also removes the old count's dependence on whether the OAuth entry is visited first. Since `checkProvidersDefined` only distinguishes zero from nonzero, no new backup behavior failure is established from that difference.

Dispatch now short-circuits OAuth reads when API-token credentials are complete, whereas the previous inline block read all five values first. Missing or invalid OAuth `_FILE` paths therefore need not produce the same reader diagnostics at that dispatch point. This follows from source; it is not a demonstrated failed backup or a separate actionable finding. Normal provider order and execution gates remain equivalent for stable configuration values.

## Verification

The offline internal package suite passed. Existing no-provider and Azure file-reader tests passed. The live credential tests skipped. There is no targeted test of partial grouped configuration, the independent rule's blank-versus-absent distinction, or the extracted validator helpers in the repository. The proposed scanner has not been compiled or run. Its behavior equivalence is assessed against the branches and truth table above; future implementation should add the focused cases listed in the summary.

Source measurements show 23 to 42 top-level functions in `backup.go` and 729 to 774 total lines. The validator shrinks individually but becomes three functions. Commands used for measurements were `git show main:internal/backup.go` and Python line/function counts over that blob and the head file; function counts use `^func `, not a complexity-score approximation. No claim is made about a measured Sonar cognitive score.
