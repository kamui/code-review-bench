# Backup tests

## Scope and measurements

Inspected `git diff main...review-head -- internal/backup_test.go`, the Gitea test and helpers at head lines 600–681, their base equivalents, the removed GitLab test, and the test environment/cleanup functions. `wc -l` and a Python line/function count over the committed base and current head establish that the file shrinks from 1016 to 989 lines while top-level function declarations increase from 37 to 39. This PR reduces an already-large file below the skill's threshold. A threshold-crossing or file-explosion finding would be unsupported.

The removed `TestPublicGitLabRepositoryBackup2` body was compared to `TestPublicGitLabRepositoryBackup` at the base, normalizing only the function name. They are identical. Deletion removes a duplicate invocation of the same live test; it does not remove a distinct assertion or scenario.

## Gitea organization scenarios

Actionable finding: the extraction at lines 651–681 duplicates the complete org-two assertion in two scenario helpers, while lines 626–633 still dispatch by selector. This preserves the scenario control structure rather than representing the changing part as expected results. A single expected-tree assertion can eliminate the switch and both overlapping assertion bodies.

Both helpers assert the org-two directory exists, read it, require exactly two entries, and require a prefix match for each of `soba-org-two-repo-one` and `soba-org-two-repo-two`. Their difference is entirely the expectation for org one: absent for the explicit org-two selector, present with exactly one matching entry for the wildcard selector. No different algorithm is needed for those cases.

The prefix search extraction itself is appropriate. `dirHasEntryWithPrefix` replaces repeated flags with an early-return predicate, and its callers retain entry-count assertions. Keep this helper or an equally direct predicate; do not weaken checks to mere existence or exact timestamped filenames.

A worked proposal makes the selectors and expected output explicit:

```go
cases := []struct {
    selector string
    repos    map[string][]string
}{
    {
        selector: sobaOrgTwo,
        repos: map[string][]string{
            sobaOrgTwo: {"soba-org-two-repo-one", "soba-org-two-repo-two"},
        },
    },
    {
        selector: "*",
        repos: map[string][]string{
            sobaOrgOne: {"soba-org-one-repo-one"},
            sobaOrgTwo: {"soba-org-two-repo-one", "soba-org-two-repo-two"},
        },
    },
}
for _, tc := range cases {
    require.NoError(t, os.Setenv(envGiteaOrgs, tc.selector))
    require.NoError(t, Run())
    assertGiteaBackupTree(t, tc.repos)
    resetBackups()
}
```

The expected-tree helper has one directory-read and one prefix-check algorithm:

```go
func assertGiteaBackupTree(t *testing.T, expected map[string][]string) {
    t.Helper()
    root := path.Join(os.Getenv(envGitBackupDir), "gitea.lessknown.co.uk")
    for _, org := range []string{sobaOrgOne, sobaOrgTwo} {
        dir := path.Join(root, org)
        prefixes, present := expected[org]
        if !present {
            require.NoDirExists(t, dir)
            continue
        }
        require.DirExists(t, dir)
        entries, err := os.ReadDir(dir)
        require.NoError(t, err)
        require.Len(t, entries, len(prefixes))
        for _, prefix := range prefixes {
            require.True(t, dirHasEntryWithPrefix(entries, prefix),
                "missing repository prefix %q in %s", prefix, dir)
        }
    }
}
```

Presence in the expected map explicitly means an organization directory should exist; absence means it should not. The helper checks both known fixture organizations, not only expected keys, so it retains the negative assertion. The map is a concrete fixture model rather than an optional production mode. Iterating over the fixed organization list also gives deterministic assertion ordering.

Retain the test's outer deferred cleanup so `require` failures do not leave the backup directory populated. One normal cleanup per iteration is sufficient. The case-local cleanup followed by unconditional cleanup already existed in the base; it is not a newly introduced defect and is not a separate finding here. The proposal collapses it as part of expressing one scenario lifecycle.

This alternative changes the order of some equivalent assertions and adds useful failure context, but retains all current directory-presence, entry-count, and prefix checks. The two cases still run in order against a clean backup directory. No changes to provider credentials, environment restoration, or live-host requirements are included. The sketch was not applied or executed.

## Verification status

The allowed offline internal test run passed. The changed Gitea organization test skipped because its token was unavailable. Its helper equivalence is a static conclusion: the new helpers preserve the original assertion set, including org-one absence in the explicit case. The removed duplicate was mechanically compared to the retained base test. The local GitHub mock test passed and verifies fixture clone/bundle behavior, not Gitea organization selection.

Credential-backed execution is still needed to validate the Gitea scenario after implementing the proposal. The existing test changes do not establish broader behavior-preservation coverage for the extracted production helpers. No additional test run was performed just to repeat a passing flag set.
