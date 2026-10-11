# Test extraction and coverage ownership

## Evidence and measurements

`git diff main...review-head -- internal/backup_test.go` shows 45 additions and 72 deletions. `git show main:internal/backup_test.go | wc -l` reports 1,016 lines and `wc -l internal/backup_test.go` reports 989. This moves an already large file below 1,000 lines, rather than crossing the skill's upper threshold.

The diff removes `TestPublicGitLabRepositoryBackup2`. Comparing its removed body with the surviving `TestPublicGitLabRepositoryBackup` at head lines 551–568 confirms the same credential skip, webhook clearing, environment backup/restoration, preflight/reset, excluded environment keys, and `Run` assertion. Deleting it does not remove a distinct scenario.

The Gitea organisation test is at head lines 600–637. It retains the same token and API URL prerequisites, environment setup, two filter values, backup calls, and cleanup behavior. The old inline boolean loops are replaced by `dirHasEntryWithPrefix` at 639–649 and two assertion helpers at 651–681. Both assertion helpers correctly call `t.Helper()`, and the new prefix helper is pure. The same directory cardinalities and repository prefix checks survive the refactor.

## Finding: mode-specific helpers duplicate the same organisation invariant

`assertGiteaOrgTwoOnlyBackedUp` asserts that organisation two exists, reads its directory, requires exactly two entries, and requires both known repository prefixes. `assertGiteaAllOrgsBackedUp` repeats all of those checks at lines 668 and 672–679 while adding the organisation-one checks. The caller chooses between the helpers using a switch at lines 626–633. Each helper has exactly one caller.

This is a missed simplification in the changed test abstraction. The repeated assertions existed inline before the PR; extracting them into two mode-specific helpers preserves duplication and makes the organisation-two invariant have two named owners. Updating its fixture count or repository names still requires editing both bodies. The new prefix helper removes scanning boilerplate but does not address the repeated directory/read/count/assert sequence. The correct abstraction is one expected organisation backup, with the case supplying existence and expected repositories.

## Worked code-judo proposal

Keep the outer setup intact. Express the two cases as expected directories and expected absences, and use one assertion helper for an organisation's contents. No reflection, generic assertion framework, or provider abstraction is necessary.

```go
type orgBackupExpectation struct {
    org      string
    prefixes []string
}

orgTwo := orgBackupExpectation{
    org: sobaOrgTwo,
    prefixes: []string{"soba-org-two-repo-one", "soba-org-two-repo-two"},
}
orgOne := orgBackupExpectation{
    org: sobaOrgOne,
    prefixes: []string{"soba-org-one-repo-one"},
}

cases := []struct {
    filter  string
    present []orgBackupExpectation
    absent  []string
}{
    {filter: sobaOrgTwo, present: []orgBackupExpectation{orgTwo}, absent: []string{sobaOrgOne}},
    {filter: "*", present: []orgBackupExpectation{orgOne, orgTwo}},
}

for _, tc := range cases {
    require.NoError(t, os.Setenv(envGiteaOrgs, tc.filter))
    require.NoError(t, Run())
    for _, expected := range tc.present {
        assertGiteaOrgBackedUp(t, expected.org, expected.prefixes)
    }
    for _, org := range tc.absent {
        require.NoDirExists(t, path.Join(os.Getenv(envGitBackupDir), "gitea.lessknown.co.uk", org))
    }
    resetBackups()
}

func assertGiteaOrgBackedUp(t *testing.T, org string, prefixes []string) {
    t.Helper()
    dir := path.Join(os.Getenv(envGitBackupDir), "gitea.lessknown.co.uk", org)
    require.DirExists(t, dir)
    entries, err := os.ReadDir(dir)
    require.NoError(t, err)
    require.Len(t, entries, len(prefixes))
    for _, prefix := range prefixes {
        require.True(t, dirHasEntryWithPrefix(entries, prefix),
            "missing repository prefix %q in %s", prefix, dir)
    }
}
```

This removes the mode dispatch switch and the two mode-specific assertion bodies. Organisation two's expected repositories are declared once and consumed in both cases. Empty `absent` means there are no negative directory assertions for that case; it is a collection of expectations, not a nullable execution mode. Exact cardinality remains mandatory, so passing prefix checks alone cannot hide extra backup entries.

The proposal keeps the same two backup operations and the same assertions. It can change the order in which failed assertions are reported, which is acceptable test-structure work but should not be confused with a byte-for-byte logging claim. The current test calls `resetBackups()` twice per successful iteration as well as deferring cleanup; the existing repeated call is redundant, not a newly introduced bug. A single per-case reset plus the outer deferred cleanup keeps the intended separation of fixtures. Do not use parallel subtests: environment variables and globals are shared.

## Verification

The offline internal suite passed in 5.241 seconds with the prescribed caches and flags; the exact command is recorded in [01_backup.md](01_backup.md). The live Gitea test requires unavailable token and API URL configuration, so its real-provider behavior was not verified in this run. Preservation of its assertions was checked directly against the diff. The proposed replacement was not added to the checkout, compiled as an integration change, or exercised against Gitea.

The repository already has a deterministic GitHub fixture test in `internal/github_mock_test.go`, but it invokes the dependency host directly. It does not supply Gitea organisation coverage or validate the changed `Run` orchestration. After remediation, a temporary-directory assertion test could verify exact cardinality, each repository prefix, and missing expected directories without credentials; live Gitea execution would remain a separate integration check. No unrelated test expansion is required to accept the duplicate GitLab test removal.
