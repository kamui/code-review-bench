# Backup tests and verification

## Assessment

No separate actionable finding in this subsystem. The removal of the duplicate
GitLab test is justified, and the Gitea assertion extraction reduces local
branching without losing the original checks. This conclusion is based on the
committed source comparison; the changed credentialed Gitea test skipped.

Commands used for evidence included:

```sh
git diff main...review-head -- internal/backup_test.go
git show main:internal/backup_test.go
rg -n 'TestMain|WriteFile|Mkdir|RemoveAll|exec.Command|httptest|checkProvider' internal
```

The test file shrinks from 1,016 to 989 lines. It has 37 top-level `func`
declarations at base and 39 at head: one duplicate test removed and three helper
functions added. It moves downward through the skill's 1,000-line threshold.

## Removed GitLab test

At base, `TestPublicGitLabRepositoryBackup` and
`TestPublicGitLabRepositoryBackup2` have identical bodies: missing-token skip,
webhook unset, environment backup/restore, preflight, global reset, deferred
backup reset, the same environment exclusions, and `require.NoError(t, Run())`.
The first remains. Deleting the second removes repeated execution rather than a
distinct behavior case. No loss of a different token, provider mode, or backup
retention assertion was found.

## Gitea organization assertions

The single-organization case still requires organization two's directory,
requires organization one's directory to be absent, reads organization two's
entries successfully, requires exactly two entries, and finds both original
repository prefixes. The wildcard case still requires both directories, reads
both successfully, requires one entry in organization one and two in
organization two, and finds all three original prefixes.

The new `dirHasEntryWithPrefix` is the useful code-judo move here. The old code
maintained multiple found booleans while scanning directory entries; the new
code turns each question into a direct predicate:

```go
require.Len(t, entriesOrgTwo, 2)
require.True(t, dirHasEntryWithPrefix(entriesOrgTwo, "soba-org-two-repo-one"))
require.True(t, dirHasEntryWithPrefix(entriesOrgTwo, "soba-org-two-repo-two"))
```

That is the worked simplification already implemented in the PR. Repeated
small scans are harmless for these one- and two-entry fixtures. Returning early
on the first prefix match is equivalent to setting a boolean when any match
exists. The helper does not check whether an entry is a directory, but neither
did the old loops.

The assertion helpers both call `t.Helper()`, so failures point back to the
calling test. They repeat some fixture-specific setup for organization two,
but a generic organization-fixture model would not materially simplify this
small test enough to require another redesign.

Each switch case still calls `resetBackups`, followed by an unconditional call
after the switch. Direct comparison with base shows both resets already existed.
The PR neither introduces a second reset nor changes the cleanup order. It is
not an actionable PR finding here. Deferred cleanup still covers an assertion
failure that exits before the end of the loop.

## Executed verification

Ran the permitted internal package suite exactly once with this flag set from
the clone root:

```sh
env -u GIT_BACKUP_DIR \
  GOMODCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-035/clone-cache/gomodcache \
  GOCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-035/clone-cache/gocache \
  GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local \
  go test ./internal/ -count=1 -v -timeout=240s
```

The command returned exit code 0 and:

```text
PASS
ok  github.com/jonhadfield/soba/internal  5.205s
```

`GIT_BACKUP_DIR` was removed from the test process environment so `TestMain`'s
preflight would create its backup directory in the supplied temporary area
rather than reuse and empty an inherited path. Fixture files and test output
directories were created outside the checkout.

Most live-provider tests skipped for absent credentials or the existing
GitHub opt-in flag, including `TestGiteaOrgsRepositoryBackup`. The existing
`TestGithubRepositoryBackupWithInvalidToken` does not use that opt-in gate: it
sets an invalid token, dials `github.com:443`, and executes the host backup.
It reached GitHub and received a Bad credentials / 401 response. Therefore the
test execution was not wholly offline even with `GOPROXY=off`. This was target
test traffic; no PR discussion, review, or reference answer was fetched.

The local GitHub fixture test passed in 5.12 seconds and produced both bundles.
It calls the dependency's `NewGitHubHost` directly, bypassing `Run`,
`collectProviderBackupResults`, and the application `GitHub` adapter. It is
good local integration coverage of that host, but cannot be cited as runtime
verification of the new provider-selection loop.

The empty-provider failure, GitHub organization/token dependency, missing
backup-directory error, Azure credential-file precedence, environment/file
reader, utility, encryption-input, and mocked webhook checks passed. The
encryption tests mostly construct dependency inputs or read environment values;
they do not verify the changed orchestration.

There are no direct test calls to the new collection, startup-log, scheduled-job,
or notification-title helpers. Those changes were verified by static comparison
only. No package tests were repeated, and no credentialed integration run,
Docker build, lint, vet, or independent build command was performed.

## Checkout integrity

`git diff --check main...review-head` returned success. After the package test,
`git status --porcelain=v1 --untracked-files=all` produced no output and
`git diff --exit-code HEAD` returned success. Head remained
`136a4850df9aec8cf8813b27ba1533d4a17bc642`, whose tree is
`d50974711ad3921de8e81016e044188ad8bd0358`. Report files live exclusively under
the requested work-directory report path; no test, source, or remedy was added
to the clone.
