# Backup test refactor

## Scope and measurements

`git diff main...review-head -- internal/backup_test.go` is +45/−72.
`git show main:internal/backup_test.go | wc -l` gives 1,016 lines; the
head has 989. The PR brings this file below 1,000 lines rather than
crossing upward. Function count changes from 37 to 39, measured with
`rg -c '^func '` against the base blob and the head file.

There are no additional actionable findings in this subsystem. The
assertion helpers preserve the checks and use `t.Helper()` appropriately.
The boolean prefix helper owns a real repeated search operation.

## Removed GitLab duplicate

Direct comparison of the base source at lines 551–590 shows that
`TestPublicGitLabRepositoryBackup2` has the same token guard, webhook
unset, environment backup and restore, preflight, global reset, backup
cleanup, environment exceptions, and `Run` assertion as the preceding
`TestPublicGitLabRepositoryBackup`. Only its function name differs.
Removing it removes duplicate execution without deleting a distinct
input or expectation. This was verified from source, not by invoking
GitLab.

## Gitea assertion equivalence

The refactored test at head lines 600–637 retains its token and API URL
guards, setup and restoration, organization order, `Run` call, switch,
and cleanup calls. The old duplicated `resetBackups` calls remain in the
same positions; they were not introduced by this PR.

| Case | Directory expectations | Entry expectations |
| --- | --- | --- |
| `sobaOrgTwo` | Org two exists, org one absent | Org two has exactly two entries; both original repository prefixes present |
| `"*"` | Both org directories exist | Org one has exactly one entry; org two has exactly two; all three original repository prefixes present |

`assertGiteaOrgTwoOnlyBackedUp`, lines 651–663, contains the first case's
unchanged expectations. `assertGiteaAllOrgsBackedUp`, lines 665–681,
contains the second case's unchanged expectations. Both use `require`
and mark themselves as helpers. `dirHasEntryWithPrefix`, lines 641–649,
returns true on the first match instead of setting a boolean and
finishing the loop. Directory entry inspection has no relevant side
effect, so this preserves the existential assertion.

## Code-judo alternative considered

A data-driven expectation could delete the scenario switch and the two
scenario-specific assertion functions. For example, describe both cases
using the same shape:

```go
cases := []struct {
    org      string
    expected map[string][]string
}{
    {sobaOrgTwo, map[string][]string{
        sobaOrgTwo: {"soba-org-two-repo-one", "soba-org-two-repo-two"},
    }},
    {"*", map[string][]string{
        sobaOrgOne: {"soba-org-one-repo-one"},
        sobaOrgTwo: {"soba-org-two-repo-one", "soba-org-two-repo-two"},
    }},
}
```

An `assertGiteaOrgBackups(t, expected)` helper would iterate the fixed
ordered list `{sobaOrgOne, sobaOrgTwo}`. An organization absent from the
map would require `NoDirExists`; a present organization would require
`DirExists`, successful `ReadDir`, `Len(entries, len(prefixes))`, and
every prefix's presence using `dirHasEntryWithPrefix`. The test would set
the case's organization filter, run the backup, assert that expectation,
and perform the original cleanup.

This shape retains negative directory assertions and exact counts;
merely iterating expected prefixes would lose important coverage. It
also avoids nondeterministic map iteration in the assertions. However,
for two fixed scenarios the existing named helpers are legible and the
alternative adds an expectation model. It is not a required remedy or
a separate finding. The code sketch was not applied or executed.

## Verification limits

The selected offline test invocation described in `01_backup.md` passed
20 top-level tests, including existing configuration and environment/file
cases. `TestGiteaOrgsRepositoryBackup` was excluded from that selection;
credentials and the live Gitea service are unavailable. Its assertions
were checked against the base diff statically. No successful execution
of the changed helpers is claimed.

The local GitHub fixture passes but constructs the dependency host
directly. It should not be described as coverage of every provider gate
or of the newly extracted `collectProviderBackupResults`. The existing
suite also provides no direct selected coverage of partial paired
credentials or the notification status helper. Those limitations qualify
the verification result; they do not themselves establish a regression
in this assertion refactor.
