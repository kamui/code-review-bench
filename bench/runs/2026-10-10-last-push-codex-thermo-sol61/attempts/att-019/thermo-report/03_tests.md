# Test refactoring and executed verification

## Source review

Reviewed `git diff main...review-head -- internal/backup_test.go`, the full
changed test file, and supporting utility, environment-file, encryption,
webhook, and local GitHub fixture tests. `backup_test.go` decreases from
1,016 lines to 989. It does not cross the skill's forbidden growth threshold;
it moves back below it. No standalone file-size finding is justified.

The removed `TestPublicGitLabRepositoryBackup2` matches
`TestPublicGitLabRepositoryBackup` in credential guard, webhook clearing,
environment preservation, preflight/reset operations, deferred cleanup,
environment selection, and `Run` assertion. Its removal loses no distinct
scenario. Both were live tests rather than independent deterministic coverage.

The Gitea test still runs the selected organization and wildcard scenarios.
`assertGiteaOrgTwoOnlyBackedUp` checks that organization two exists and
organization one does not, reads the former directory, checks two entries,
and requires the two repository prefixes. `assertGiteaAllOrgsBackedUp` checks
both directories, reads each, requires one and two entries respectively,
and checks all three repository prefixes. The new prefix helper returns on
the first match rather than retaining three separate discovery flags.

The assertion functions use `t.Helper`, so assertion failures point back
to the scenario call. The count assertions remain important: prefix-only
tests could accept unexpected extra backups. The extraction retains them.
Separate scenarios still duplicate some organization-two assertions, and
cleanup remains duplicated inside and after the switch. Both repetitions
were already present. They are not significant enough to compete with the
structural findings in the summary.

A possible future local simplification would represent expected directories
and repository prefixes as scenario data, then have one assertion helper
read a directory and check its exact entry count plus required prefixes.
Such a helper should also retain the selected scenario's explicit assertion
that the excluded organization is absent. This is an optional direction,
not an additional actionable finding in this review. The current small
assertion helpers are understandable and preserve the old assertions.

## Executed command and limits

The following command was run once from the clone root. The environment was
cleared except for the executable search path and prescribed Go settings,
preventing ambient credentials, backup paths, or notification URLs from
affecting execution. `TestMain` creates a temporary backup directory when no
backup path is set. No scratch files were added to the checkout.

```sh
env -i PATH="$PATH" \
  GOMODCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-019/clone-cache/gomodcache \
  GOCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-019/clone-cache/gocache \
  GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local \
  timeout 300 go test ./internal/ -count=1 -timeout=240s \
  -skip '^TestGithubRepositoryBackupWithInvalidToken$' -v
```

Output was redirected to `../thermo-go-test.log` relative to this report
directory. Exit status was 0. The package reported 5.161 seconds, with 23
top-level tests passing, 15 skipping, and zero failures. Counts exclude
subtest rows. The explicitly excluded invalid-token test is additional to
the 15 skips; it does not appear as a skipped row in the test log.

The excluded test calls `DialContext` against `github.com:443` and then uses
the real GitHub host. It was excluded to obey the offline execution policy.
All live provider tests skipped because required credentials or the live
GitHub opt-in were absent. In particular, the changed Gitea organization
test and surviving live GitLab test were compiled but skipped.

`TestGithubRepositoryBackupWithMockAPI` passed in 5.14 seconds. It exercises
repo enumeration, local HTTP cloning, and bundle creation with temporary git
fixtures. It constructs a githosts host directly; it does not exercise the
new `collectProviderBackupResults` dispatch loop. The webhook tests use
gock interception and passed without live webhook requests. The credential
file and startup rejection tests passed.

This verification does not cover real provider backups, actual Slack/ntfy/
Telegram transport payloads, interval/cron lifecycle, or the complete auth
validation matrix. None of those unexecuted paths is claimed to be tested.
No full build, vet, lint, race run, or Docker build was performed; the PR's
body claims those checks, but this review did not independently reproduce
them under the narrower execution allowance.

## Checkout integrity

`git diff --check main...review-head` returned 0 without output. Before
testing, every tracked file listed by `git ls-files -z` was SHA-256 hashed
into `../thermo-tree-before.json`. After testing, all tracked-file hashes
matched. `git status --porcelain=v1` was empty, and HEAD remained the pinned
`136a4850df9aec8cf8813b27ba1533d4a17bc642`. Review artifacts are outside the
clone. No remedies were applied.
