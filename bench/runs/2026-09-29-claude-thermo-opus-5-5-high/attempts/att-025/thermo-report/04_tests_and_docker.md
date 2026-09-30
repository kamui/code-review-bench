# 04 — Test refactor (`internal/backup_test.go`) and Dockerfile

Commands used:

```
git diff main...review-head -- internal/backup_test.go docker/Dockerfile
sed -n 545,570p internal/backup_test.go     # surviving TestPublicGitLabRepositoryBackup
sed -n 600,681p internal/backup_test.go     # TestGiteaOrgsRepositoryBackup + helpers
go test ./internal/ -count=1                 # ok; live Gitea/GitLab tests skip without credentials
```

Measurements: `internal/backup_test.go` goes from 1016 to 989 lines, below the 1k threshold. Removing `TestPublicGitLabRepositoryBackup2` is correct: its body was byte-identical to `TestPublicGitLabRepositoryBackup` (backup_test.go:551-568).

## Finding T1 — `TestGiteaOrgsRepositoryBackup` was split into two near-copy assertion helpers instead of being made table-driven, and it keeps the triple `resetBackups()`

**Where:** `internal/backup_test.go:622-681`.

**Evidence.**

- The loop body still calls `resetBackups()` inside each `case` (lines 629 and 632) and then again unconditionally after the `switch` (line 635), with a `defer resetBackups()` as well (line 617). Every iteration resets twice. The refactor touched every one of these lines and kept the redundancy.
- `assertGiteaOrgTwoOnlyBackedUp` and `assertGiteaAllOrgsBackedUp` repeat `path.Join(os.Getenv(envGitBackupDir), "gitea.lessknown.co.uk", <org>)` six times between them. The second helper is the first helper plus one more org. The `switch org` exists only to choose which helper to call, so the test's data (org filter → expected dirs and repo prefixes) is encoded as control flow.

**Remedy (code-judo).** Express the expectation as data and delete the `switch` and both bespoke helpers:

```go
giteaOrgDir := func(org string) string {
    return path.Join(os.Getenv(envGitBackupDir), "gitea.lessknown.co.uk", org)
}

cases := []struct {
    filter  string
    want    map[string][]string // org -> expected repo-name prefixes (exact entry count = len)
    absent  []string
}{
    {sobaOrgTwo, map[string][]string{sobaOrgTwo: {"soba-org-two-repo-one", "soba-org-two-repo-two"}}, []string{sobaOrgOne}},
    {"*", map[string][]string{
        sobaOrgOne: {"soba-org-one-repo-one"},
        sobaOrgTwo: {"soba-org-two-repo-one", "soba-org-two-repo-two"},
    }, nil},
}

for _, tc := range cases {
    require.NoError(t, os.Setenv(envGiteaOrgs, tc.filter))
    require.NoError(t, Run())
    for org, prefixes := range tc.want {
        entries, err := os.ReadDir(giteaOrgDir(org))
        require.NoError(t, err)
        require.Len(t, entries, len(prefixes))
        for _, p := range prefixes {
            require.True(t, dirHasEntryWithPrefix(entries, p), "missing %s in %s", p, org)
        }
    }
    for _, org := range tc.absent {
        require.NoDirExists(t, giteaOrgDir(org))
    }
    resetBackups()
}
```

`os.ReadDir` succeeding already implies `DirExists`, so the separate `require.DirExists` calls can go. The result has one reset per iteration, one assertion path, and adding an org case is a one-row change. `dirHasEntryWithPrefix` is a good extraction and stays.

**Verification status:** Confirmed by reading. The test requires live Gitea credentials and skips in this environment, so neither the current nor the proposed form could be executed here.

## Finding D1 — The rewritten `RUN` keeps a no-op cleanup step

**Where:** `docker/Dockerfile:5-8`.

**Evidence.** `rm -f "/var/cache/apk/*"` quotes the glob, so the shell looks for a file literally named `*` and removes nothing. The step is also redundant, because `apk add --no-cache` does not populate `/var/cache/apk`. The PR rewrote this exact `RUN` instruction (merging two layers and re-sorting packages) and carried the dead command forward.

**Remedy.** Delete `&& rm -f "/var/cache/apk/*"`. Behaviour is unchanged: image contents are identical, and the command stops pretending to do something.

**Verification status:** I checked the shell semantics by reading. Docker is unavailable, so I did not build the image. The other Dockerfile changes (layer merge, sorted packages, quoted URL) are correct.
