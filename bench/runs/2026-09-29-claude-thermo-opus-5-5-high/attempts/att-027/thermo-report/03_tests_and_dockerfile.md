# 03 — Tests (`internal/backup_test.go`) and `docker/Dockerfile`

## Commands run

```
git diff main...review-head -- internal/backup_test.go docker/Dockerfile
git show main:internal/backup_test.go | wc -l     # 1016 -> 989 at head
go test ./internal/ -count=1                      # ok
go test ./internal/ -count=1 -coverprofile=$TMP/cover.out
go tool cover -func=$TMP/cover.out
```

Docker is unavailable in this environment, so the Dockerfile was reviewed by reading only.

---

## Finding 3.1 — The refactor's new seams have no offline coverage

**Evidence (per-function coverage at head, offline, live-provider tests skipped):**

| Function | Coverage |
| --- | --- |
| `collectProviderBackupResults` (`backup.go:97`) | 0.0% |
| `createWorkingDir` (`backup.go:473`) | 0.0% |
| `scheduleBackups` (`backup.go:491`) | 0.0% |
| `runScheduledJob` (`backup.go:525`) | 0.0% |
| `backupStatusTitle` (`notify.go:31`) | 0.0% |
| `logProviderBackupsToKeep` (`backup.go:210`) | 0.0% |
| `displayGitLabStartupConfig` (`backup.go:259`) | 22.2% |
| `displayGiteaStartupConfig` (`backup.go:248`) | 33.3% |
| `validateStartupConfig` (`backup.go:443`) | 46.7% |

The PR body asserts "Behaviour and log output are unchanged", but nothing in the suite that runs without credentials checks that claim. The decomposition made several of these units easy to test: `backupStatusTitle` is pure; `collectProviderBackupResults` and `displayStartupConfig` depend only on env vars and the package `logger`, which a test can point at a `bytes.Buffer`. A golden-output test of `displayStartupConfig` across a few env combinations would lock down "log output unchanged" at low cost. It would also make the registry refactor in `01_backup_orchestration.md` safe to carry out. A four-row table test for `backupStatusTitle` (or `classifyBackups`, per `02_notify.md`) is trivial.

**Verification status.** Measured (numbers above). The test suite passes.

---

## Finding 3.2 — `TestGiteaOrgsRepositoryBackup` refactor keeps redundant resets and two near-duplicate assertion helpers

**Where.** `internal/backup_test.go:600-679`.

**Evidence.**

- Each `switch` arm calls `resetBackups()`, and the loop body then calls `resetBackups()` again unconditionally (`backup_test.go:626-636`). That is two resets per iteration plus the deferred one. The PR rewrote these exact lines and kept the redundancy.
- `assertGiteaOrgTwoOnlyBackedUp` and `assertGiteaAllOrgsBackedUp` build the same `path.Join(os.Getenv(envGitBackupDir), "gitea.lessknown.co.uk", <org>)` six times between them, and differ only in which org directories and entry prefixes are expected.
- The `switch org` inside a loop over a two-element literal is a hand-rolled table.

**Code-judo proposal.** Make the expectation the table row:

```go
cases := []struct {
	orgs    string
	present map[string][]string // org dir -> required entry prefixes
	absent  []string
}{
	{sobaOrgTwo, map[string][]string{sobaOrgTwo: {"soba-org-two-repo-one", "soba-org-two-repo-two"}}, []string{sobaOrgOne}},
	{"*", map[string][]string{
		sobaOrgOne: {"soba-org-one-repo-one"},
		sobaOrgTwo: {"soba-org-two-repo-one", "soba-org-two-repo-two"},
	}, nil},
}
for _, tc := range cases {
	require.NoError(t, os.Setenv(envGiteaOrgs, tc.orgs))
	require.NoError(t, Run())
	assertGiteaBackups(t, tc.present, tc.absent) // one helper: DirExists/NoDirExists, Len, dirHasEntryWithPrefix
	resetBackups()
}
```

This deletes one of the two assertion helpers and the `switch`, and leaves a single reset per iteration.

**Verification status.** Confirmed by reading. The test itself skips without Gitea credentials, so the refactor's equivalence could not be executed. By reading, the assertions are carried over one-for-one.

**Removed duplicate test.** `TestPublicGitLabRepositoryBackup2` was byte-identical in body to `TestPublicGitLabRepositoryBackup` (`backup_test.go:551-568`). Removing it is correct.

---

## Dockerfile (`docker/Dockerfile`)

The merged `RUN`, sorted package list and quoted URL are all fine. One leftover was touched but not fixed: line 8, `rm -f "/var/cache/apk/*"`, quotes the glob, so the shell never expands it. The command tries to remove a file literally named `*` and does nothing. It is also redundant with `apk add --no-cache`. The PR rewrote this `RUN` instruction, so this was the natural moment to delete the line. Folded into summary finding 8.
