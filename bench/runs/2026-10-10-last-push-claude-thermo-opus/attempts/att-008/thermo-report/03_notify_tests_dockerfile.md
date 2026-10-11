# 03 — `notify.go`, `backup_test.go`, `docker/Dockerfile`, and how the review was verified

Covers summary findings F8, F9, F10 and F11, and carries the measurements and the harness used across all three detail files. All line numbers are at the head, `136a485`.

## 1. Backup status titles (F8)

**Evidence.** `internal/notify.go:26-40`:

```go
	titleBackupsSucceeded = "🚀 soba backups succeeded"
	titleBackupsErrors    = "️⚠️ soba backups completed with errors"
	titleBackupsFailed    = "️🚨 soba backups failed"
)

func backupStatusTitle(succeeded, failed int) string {
	switch {
	case succeeded > 0 && failed == 0:
		return titleBackupsSucceeded
	case failed > 0 && succeeded > 0:
		return titleBackupsErrors
	default:
		return titleBackupsFailed
	}
}
```

Callers: `notify.go:104`, `191`, `216`. And `internal/backup.go:76-83`, untouched by the PR but inside `runProviderBackups`, which the PR refactored:

```go
switch {
case succeeded == 0 && failed >= 0:
	logger.Println("all backups failed")
case succeeded > 0 && failed > 0:
	logger.Println("backups completed with errors")
default:
	logger.Println("backups complete")
}
```

**What is good.** Three identical `switch` statements became one function. That is real deduplication and it is the cleanest change in the PR.

**What is left.** Three things, in decreasing order of importance.

The log `switch` in `backup.go` partitions `(succeeded, failed)` into the same three outcomes as `backupStatusTitle`, written with different predicates and in a different order. For non-negative counts, "succeeded is zero" is the failed case in both, "both positive" is the partial case in both, and the remainder is success in both. `failed >= 0` is always true. So the classification still exists twice, in two files, and they can drift.

Each sender is handed `succeeded` and `failed` and calls `backupStatusTitle` itself. The title is one fact about one run; it could be computed once.

The three constants each have exactly one use, inside the helper. Before the helper existed, constants were the way to remove the duplicated literals. With the helper, the literals would appear once each anyway, so the constants are a second layer of names. They are also appended to a `const` block that until now held only environment-variable names.

**Proposed shape** (not built):

```go
type backupOutcome int

const (
	backupsSucceeded backupOutcome = iota
	backupsPartlyFailed
	backupsFailed
)

func classifyBackups(succeeded, failed int) backupOutcome {
	switch {
	case succeeded == 0:
		return backupsFailed
	case failed > 0:
		return backupsPartlyFailed
	default:
		return backupsSucceeded
	}
}
```

with `title()` and `logLine()` methods holding the six strings, `runProviderBackups` calling it once, and `notify` passing the outcome or the title to the senders. That is one classification, stated with the simplest predicates that describe it.

**A note on the literals.** `od -c` on lines 27 and 28 shows that the "errors" and "failed" titles begin with a stray U+FE0F variation selector (`357 270 217`) before the emoji. The base literals have the same bytes, so this PR changed nothing, but a named constant is where someone would now go to fix it, and it would be a visible change to notification titles.

**Verification status.** By reading. The equivalence of the two partitions is by case analysis on non-negative integers, which is what `getBackupsStats` (`internal/webhook.go:110`) returns. The proposal was not compiled.

## 2. Gitea organisation test helpers (F9)

**Evidence.** `internal/backup_test.go:621-637`:

```go
for _, org := range []string{sobaOrgTwo, "*"} {
	require.NoError(t, os.Setenv(envGiteaOrgs, org))

	require.NoError(t, Run())

	switch org {
	case sobaOrgTwo:
		assertGiteaOrgTwoOnlyBackedUp(t)
		resetBackups()
	case "*":
		assertGiteaAllOrgsBackedUp(t)
		resetBackups()
	}

	resetBackups()
}
```

`resetBackups()` appears at lines 617 (deferred), 629, 632 and 635. The two helpers at lines 651-681 share the org-two existence check, the org-two `ReadDir`, the `Len(…, 2)` check and both org-two prefix checks. `path.Join(os.Getenv(envGitBackupDir), "gitea.lessknown.co.uk", …)` appears three times in the first and four in the second. `dirHasEntryWithPrefix` (lines 639-649) is a hand-written `slices.ContainsFunc`; `slices` is imported at line 14.

**Why this is a problem.** The extraction moved two blocks out of a `switch` and gave each a name. It did not notice that the blocks are the same assertion applied to different arguments. The names encode the scenario (`OrgTwoOnly`, `AllOrgs`) instead of the thing being asserted, which is why there are two. The redundant resets were carried across.

**Worked proposal** (applied in `scratch/proposal`, compiled and vetted):

```go
	for _, org := range []string{sobaOrgTwo, "*"} {
		require.NoError(t, os.Setenv(envGiteaOrgs, org))

		require.NoError(t, Run())

		requireGiteaOrgBackups(t, sobaOrgTwo, "soba-org-two-repo-one", "soba-org-two-repo-two")

		if org == "*" {
			requireGiteaOrgBackups(t, sobaOrgOne, "soba-org-one-repo-one")
		} else {
			require.NoDirExists(t, giteaOrgBackupDir(sobaOrgOne))
		}

		resetBackups()
	}
}

func giteaOrgBackupDir(org string) string {
	return path.Join(os.Getenv(envGitBackupDir), "gitea.lessknown.co.uk", org)
}

// requireGiteaOrgBackups asserts the org's backup directory holds exactly one
// entry per expected repository.
func requireGiteaOrgBackups(t *testing.T, org string, repos ...string) {
	t.Helper()

	entries, err := os.ReadDir(giteaOrgBackupDir(org))
	require.NoError(t, err)
	require.Len(t, entries, len(repos))

	for _, repo := range repos {
		require.True(t, slices.ContainsFunc(entries, func(e os.DirEntry) bool {
			return strings.HasPrefix(e.Name(), repo)
		}), "no backup found for %s", repo)
	}
}
```

Three helpers become two, the `switch` goes, and `backup_test.go` drops from 989 to 964 lines. The separate `require.DirExists` is subsumed by `os.ReadDir` succeeding. A failure now names the repository that was not found, which the head's bare `require.True` does not.

**Verification status.** Compiled and vetted in the scratch copy. Not executed: the test skips at lines 601-607 without `GITEA_TOKEN` and `GITEA_APIURL`, and no credentials or network are available. The assertions are equivalent by reading.

## 3. No test backs the behaviour claim (F10)

**Evidence.** `git diff main...review-head --stat -- internal/backup_test.go` shows +45 / −72 and every added line belongs to the Gitea helpers in section 2. `grep` for the refactored symbols across the test files finds only `checkProvidersDefined` at `backup_test.go:798`, inside `TestCheckProvidersFailureWhenNoneDefined`. There is no test for `displayStartupConfig`, `checkProvider`, `backupStatusTitle`, or the scheduling split. Run with `-v` in the scratch copy of the head, the package reports 15 skipped tests, every one of them a test that performs a real backup against a provider.

**Why this is a problem.** The PR's safety argument is one sentence in the description. The functions it rewrote are the ones that decide whether soba starts at all and what it tells the operator about its configuration. After this PR they are small and free of I/O beyond the environment and the logger, which is exactly when pinning them becomes cheap.

**Remedy.** Three table tests, none needing credentials:

- `backupStatusTitle` over `(0,0)`, `(0,n)`, `(n,0)`, `(n,m)`.
- `checkProvidersDefined` over: nothing set; each provider complete; Azure DevOps with only the username; BitBucket with only the e-mail; a blank `GITHUB_TOKEN`.
- `displayStartupConfig` with `logger` swapped for one writing to a buffer with no prefix or flags, asserting the exact lines per provider.

The harness in section 5 is most of the way to the third one.

**Verification status.** The gap is verified by reading the diff and the test file. The behaviour claim itself is verified by the harness.

## 4. Dockerfile (F11)

**Evidence.** `docker/Dockerfile:5-8`:

```dockerfile
RUN addgroup -S nonroot \
    && adduser -S nonroot -G nonroot \
    && apk add --update --no-cache bash ca-certificates curl git git-lfs grep jq \
    && rm -f "/var/cache/apk/*"
```

**Why this is a problem.** In `sh`, pathname expansion does not happen inside double quotes, so `rm` receives the literal path `/var/cache/apk/*`. No such file exists, and `-f` suppresses the error. The step has never removed anything. Separately, `apk add --no-cache` fetches the index into memory and does not populate `/var/cache/apk`, so there is nothing to remove, and `--update` (an alias for `--update-cache`) is redundant beside it.

The three changes the PR did make here are fine: merging the two `RUN`s, sorting the package list, and quoting the URL containing `${TAG}`. The sorted list contains the same seven packages as before.

**Remedy.**

```dockerfile
RUN addgroup -S nonroot \
    && adduser -S nonroot -G nonroot \
    && apk add --no-cache bash ca-certificates curl git git-lfs grep jq
```

**Verification status.** By reading only. Docker is unavailable in this environment, so neither the head's image nor the proposed one was built. The quoting behaviour is standard POSIX shell semantics; the statement about `--no-cache` is from knowledge of `apk`, not from a run.

## 5. How the review was verified

### File-size check

| File | base | head |
| --- | --- | --- |
| `internal/backup.go` | 729 | 774 |
| `internal/backup_test.go` | 1,016 | 989 |
| `internal/notify.go` | 248 | 238 |
| `docker/Dockerfile` | 23 | 22 |

No file crosses 1,000 lines upward. `backup_test.go` crosses it downward.

Function counts (`grep -c '^func '`): `backup.go` 23 → 42, `notify.go` 5 → 6, `backup_test.go` 37 → 39.

### Package tests at the head

Run once from the clone root, offline:

```
GOMODCACHE=<cache>/gomodcache GOCACHE=<cache>/gocache GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local \
  go test ./internal/ -count=1
ok  	github.com/jonhadfield/soba/internal	5.259s
```

`git status --short` in the clone was empty afterwards and `HEAD` is still `136a4850df9aec8cf8813b27ba1533d4a17bc642`.

### Differential harness

Nothing was added to the clone. `git archive main` and `git archive review-head` were unpacked into `clone-work/scratch/base` and `clone-work/scratch/head`, and one extra test file, `clone-work/scratch/zz_diff_test.go`, was copied into each copy's `internal/` directory.

The test seeds `math/rand` with a fixed value and, 20,000 times, clears and then randomly sets 31 provider variables (tokens, orgs, compare, backups, LFS, min access level, and the BitBucket, Azure DevOps and Sourcehut credentials) to values drawn from `""`, `" "`, `"x"`, `"refs"`, `"REFS"`, `"Refs "`, `"true"`, `"yes"`, `"N"`, `"Org-A,org-b"`, `"3"`, with a per-iteration density so that sparse and fully-populated environments both occur. For each environment it replaces `logger` with one writing to a buffer with no prefix and no flags, calls `displayStartupConfig()`, calls `checkProvidersDefined()`, sorts the error lines (map iteration makes their order vary run to run), and appends the environment, the log output and the verdict to a file.

Each copy was run with `go test ./internal/ -run 'TestZZDiff$' -count=1`. After replacing the one path that legitimately differs (each copy's own `GIT_BACKUP_DIR`), the two outputs are identical:

```
f982be59d807a596dff93a6f71078ada78c624e727006e0c0d8af70f5e243c40  norm-base.txt
f982be59d807a596dff93a6f71078ada78c624e727006e0c0d8af70f5e243c40  norm-head.txt
```

The sample is not degenerate: of the 20,000 environments 5,634 validate successfully and 1,443 produce "no providers defined"; the output contains 12,923 "is not defined" lines, 6,436 "compare method: refs" lines and 6,439 "backup LFS: true" lines.

**What the harness does not cover.** `_FILE` variables; `collectProviderBackupResults` (it would call the real providers); the scheduling paths; the notification senders; and non-ASCII values. Those parts of the equivalence claim are by reading, and the two places where reading turned up a difference (the `EqualFold` change and the short-circuited OAuth reads) are recorded in `01_provider_gating_and_validation.md`, sections 2, 4 and 5.

### Proposal check

`clone-work/scratch/proposal` is a copy of the head copy with the changes from F1, F2, F3, F4, F6, F7 and F9 applied; the patch is `clone-work/scratch/proposal.patch`. In that copy:

- `gofmt -l internal/` prints nothing, and `go build ./...` and `go vet ./internal/` succeed.
- The harness output, normalised the same way, is byte-identical to the head's.
- `go test ./internal/ -count=1` passes: 25 top-level tests pass (24 from the repository plus the harness test) and 15 skip, the same tally as the scratch copy of the head.
- `backup.go` is 750 lines with 44 functions and 11 `GetEnvOrFile` call sites; `backup_test.go` is 964 lines.

F5 and F8 were not built. F11 could not be built.
