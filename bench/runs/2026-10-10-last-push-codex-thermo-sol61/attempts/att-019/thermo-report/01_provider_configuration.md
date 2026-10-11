# Provider configuration and credential validation

## Scope and source evidence

Reviewed `internal/backup.go` against `main`, with supporting reads of
`internal/constants.go`, `internal/envfile.go`, `internal/bitbucket.go`,
`internal/azure_devops.go`, and existing tests. These supporting files were
used as code evidence only. No ambient guidance or external sources were used.

Commands used to obtain the evidence included:

```sh
git diff main...review-head -- internal/backup.go
nl -ba internal/backup.go
git show main:internal/backup.go
cat internal/constants.go
cat internal/envfile.go
cat internal/bitbucket.go
cat internal/azure_devops.go
```

`backup.go` has 729 lines and 23 functions at the base, and 774 lines and
42 functions at the head. The base `checkProvider` implementation spans
49 lines, excluding its preceding comment. At the head its implementation
and two newly extracted helper implementations occupy 64 lines in total,
excluding their comments and separating blank lines. These are physical
source measurements, not a Sonar cognitive-complexity score.

## F1: additional boundaries around duplicated scanning

The finding is anchored at `internal/backup.go:331–336`, where the new helper
calls dispatch validation. `checkJustTokenProvider` at lines 348–365 and
`checkUserAndPasswordProvider` at lines 369–395 both retrieve parameters from
`enabledProviderAuth`, read them with `GetEnvOrFile`, and classify values with
`strings.Trim(val, " ")`. Their boundary returns an integer but sends errors
back through a mutable `*strings.Builder` owned by the caller. That splits a
single validation result across two channels and requires reading both sides
of the helper boundary to understand the count/error relationship.

The grouped helper first reads all parameters to count nonblank values, then
reads them all again to identify missing parameters if some are configured.
For an Azure DevOps username present with its PAT absent, lines 372–379 read
both values, then lines 382–387 read both again. This is not a new duplicate
read introduced by the PR. It is an existing mechanism copied into the new
abstraction rather than simplified by the refactor.

One ordered collection pass can produce all the information both policies
need. The token policy uses configured count and explicitly blank parameters;
the grouped policy uses configured count and all missing parameters. That
removes duplicate traversal, repeated blank-value classification, and the
builder-mutating helper API together. The new boundaries should earn their
place through such a reduction instead of merely reducing complexity per
function as measured by a linter.

## Worked code-judo proposal

The following is a review sketch, not an applied change. It replaces the
three validation functions with one scan followed by the two existing policy
rules. It deliberately retains the existing category lists, count semantics,
ASCII-space trimming, error wording, and diagnostic ordering. A broader
provider registry is unnecessary for this simplification.

```go
func checkProvider(provider string) (int, error) {
    token := slices.Contains(justTokenProviders, provider)
    grouped := slices.Contains(userAndPasswordProviders, provider)
    if !token && !grouped {
        return 0, nil
    }

    params := enabledProviderAuth[provider]
    var configured int
    var missing, blank []string
    for _, param := range params {
        value, exists := GetEnvOrFile(param)
        if exists && strings.Trim(value, " ") != "" {
            configured++
            continue
        }

        message := fmt.Sprintf("%s parameter '%s' is not defined.\n", provider, param)
        missing = append(missing, message)
        if exists {
            blank = append(blank, message)
        }
    }

    var count int
    var messages []string
    if token {
        count = configured
        messages = append(messages, blank...)
    }
    if grouped {
        if configured == len(params) {
            count++
        } else if configured > 0 {
            messages = append(messages, missing...)
        }
    }

    if len(messages) > 0 {
        return count, errors.New(strings.Join(messages, ""))
    }
    return count, nil
}
```

The collection is data, and the two policies operate on that data. There is
one credential-read site, one blank-value predicate, and no shared mutable
error accumulator across function calls. If the project prefers named policy
functions, make those pure consumers of collected state; do not recreate
separate readers inside each policy. Remeasure actual cognitive complexity
before landing the replacement; this review did not execute Sonar or lint.

## Behavior that must survive remediation

`enabledProviderAuth` is not uniformly an all-required credential schema.
Gitea and Sourcehut are in `justTokenProviders` but each has both an API URL
and a token in its parameter list. Both nonblank parameters produce count 2.
An API URL alone can produce count 1 without an error. These are existing
semantics, even though the pre-existing `checkProvider` comment says 0 or 1.
The PR does not introduce that comment discrepancy, so it is not a finding.

The following matrix describes the current validation result on a stable
environment and stable credential files. Errors list parameters in their
declared order and retain the terminating newline.

| Policy | Collected state | Count | Validation errors |
| --- | --- | --- | --- |
| Token | All parameters unset | 0 | None |
| Token | One explicitly blank parameter | 0 | That blank parameter |
| Token | One nonblank parameter, another unset | 1 | None |
| Token | One nonblank parameter, another blank | 1 | The blank parameter |
| Token | Two nonblank parameters | 2 | None |
| Grouped | All parameters unset or blank | 0 | None |
| Grouped | Some nonblank parameters | 0 | Every unset or blank parameter |
| Grouped | All parameters nonblank | 1 | None |
| Neither category | Any state | 0 | None; no credential reads |

Only literal ASCII spaces are removed by `strings.Trim(value, " ")` for
direct environment values. `GetEnvOrFile` already applies `TrimSpace` to file
contents. Replacing the validation predicate with `TrimSpace` everywhere
would change how direct tabs/newlines are treated. Likewise, forcing token
providers to have every listed parameter would be a behavior change.

The sketch also preserves the unusual case where a provider appears in both
category lists: token counts and blank errors come first, then the grouped
count or missing errors. The current declared lists are disjoint.

## Q1 and file-read diagnostics

`GetEnvOrFile` performs I/O and logs read failures; it is not a pure lookup.
Removing the second grouped scan can reduce duplicate file-error messages.
A file that changes between the two current passes can also yield an error
list inconsistent with the first count. A single scan gives a consistent
snapshot, but that is not an assertion of exact equivalence for changing
files or for repeated low-level log emissions.

Clarify the stated log contract before claiming complete behavior preservation.
If exact repeated I/O diagnostics are required, the configuration boundary
needs an explicit diagnostic policy rather than silently changing emissions.
The proposal preserves validation results for stable inputs; its logging
contract remains the question identified in the summary.

## Accepted provider-collection changes

`collectProviderBackupResults` keeps execution order Bitbucket, Gitea,
GitHub, GitLab, Azure DevOps, Sourcehut. The new table at lines 105–114
replaces repeated activation checks with a small, explicit ordered list.
Azure DevOps intentionally remains gated by username there, with PAT
validation elsewhere, matching the original flow. Treating every activation
key as a token would be incorrect; the table's data itself makes the current
choice visible. This naming issue is not raised as a separate finding.

Bitbucket completeness helpers are reused by collection and configuration
validation. Computing API-token completeness before iterating the map avoids
counting two complete authentication methods according to map iteration
order. The previous count only affected zero/nonzero validation, so no new
public error behavior follows from that internal counting correction.

Collection now short-circuits the OAuth predicate when API-token credentials
are complete. The `Bitbucket` provider still reads both credential groups
when invoked. This can affect repeated `_FILE` diagnostics but does not
change selected authentication for stable inputs. Provider parallelization
would introduce new questions about shared clients and backup filesystem
ownership and is not necessary to improve this refactor.

## Verification status

The offline internal package passed; source comparison supports the stated
control-flow equivalence. There is no direct test of `checkProvider`'s full
partial/blank credential matrix in the inspected tests. The Azure credential
file test checks `GetEnvOrFile`, not the extracted grouped validator. The
no-providers test checks the aggregate empty configuration. Therefore the
test pass is not evidence that every changed validation path was exercised.

The worked proposal was not installed, compiled, linted, or run. Its result
matrix was checked against source semantics. Meaningful validation before
landing it should cover the matrix above and `_FILE` failure diagnostics,
with no live provider calls needed.
