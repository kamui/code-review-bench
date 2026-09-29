# Detail 02: docker/Dockerfile and internal/backup_test.go

## Finding 8: `TestGiteaOrgsRepositoryBackup` (`internal/backup_test.go:600-680`)

Current shape after the PR:

```go
for _, org := range []string{sobaOrgTwo, "*"} {
    require.NoError(t, os.Setenv(envGiteaOrgs, org))
    require.NoError(t, Run())
    switch org {
    case sobaOrgTwo: assertGiteaOrgTwoOnlyBackedUp(t); resetBackups()
    case "*":        assertGiteaAllOrgsBackedUp(t);     resetBackups()
    }
    resetBackups()
}
```

Each iteration runs `resetBackups()` twice. The pre-PR version had the same double reset, so this is not a regression, but the PR rewrote every other line of the block and left both calls in place. The automated review flagged the same thing on this line.

Worked proposal:

```go
cases := []struct{ org string; want map[string][]string }{
    {sobaOrgTwo, map[string][]string{sobaOrgTwo: {"soba-org-two-repo-one", "soba-org-two-repo-two"}}},
    {"*", map[string][]string{
        sobaOrgOne: {"soba-org-one-repo-one"},
        sobaOrgTwo: {"soba-org-two-repo-one", "soba-org-two-repo-two"}}},
}
for _, c := range cases { set env; Run(); assertOrgDirs(t, c.want, c.org != "*"); resetBackups() }
```

`assertOrgDirs` asserts each wanted directory exists with exactly `len(prefixes)` entries and every prefix present, and, for the single-org case, that the other org directory does not exist. This removes the `switch`, both wrappers, the duplicate reset and the repeated `path.Join(os.Getenv(envGitBackupDir), "gitea.lessknown.co.uk", ...)` calls. `dirHasEntryWithPrefix` is reasonable and can stay.

Verification status: the credentialed test skips here, so neither the current nor the proposed version could be executed; `go vet` and compilation of the current version passed.

Removal of `TestPublicGitLabRepositoryBackup2`: verified identical to `TestPublicGitLabRepositoryBackup` in body (same skip, env handling, `unsetEnvVarsExcept` list and assertion). The removal is correct.

## Finding 9: Dockerfile (`docker/Dockerfile:5-9`)

After the PR:

```
RUN addgroup -S nonroot \
    && adduser -S nonroot -G nonroot \
    && apk add --update --no-cache bash ca-certificates curl git git-lfs grep jq \
    && rm -f "/var/cache/apk/*"
```

- The quoted glob means `rm` looks for a file literally called `*`; it is a no-op. `--no-cache` already avoids writing an apk cache, so the line has no purpose.
- `--update` is redundant next to `--no-cache`.
- Merging the user creation and package install into one layer is fine for S7031. Quoting `${TAG}` in the URL is a correct S6570 fix. Package names are sorted as requested by S7018.
- The image was not built (Docker unavailable); verification is by reading.
