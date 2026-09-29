# Review blind-44e20d

### Item 1
Location: internal/backup.go:360
Claim: checkJustTokenProvider counts every non-empty auth parameter, so GITEA_APIURL or SOURCEHUT_APIURL alone is treated as a fully configured provider even though no token is set.
Consequence: Set only GIT_BACKUP_DIR and GITEA_APIURL (no GITEA_TOKEN). checkProvider returns 1, checkProvidersDefined passes instead of returning 'no providers defined', collectProviderBackupResults runs nothing because it gates on GITEA_TOKEN, getBackupsStats returns 0/0, the run logs 'all backups failed' and sends a 'failed' notification, but execProviderBackups exits 0 because failed == 0. The helper can also return 2 for Gitea/Sourcehut, contradicting checkProvider's documented '0 or 1'.
Fix: —

### Item 2
Location: internal/backup.go:689
Claim: checkProvidersDefined special-cases both BitBucket providers and never calls checkProvider for them, so partially configured BitBucket credentials are never reported, unlike Azure DevOps.
Consequence: Set BITBUCKET_EMAIL but omit BITBUCKET_API_TOKEN (or set BITBUCKET_USER/KEY without SECRET) alongside a valid GITHUB_TOKEN. Validation passes with no error, BitBucket is silently skipped on every run and the user gets no 'parameter ... is not defined' message. With no other provider the only message is 'no providers defined'. The BitBucket entries in userAndPasswordProviders are dead as a result.
Fix: —

### Item 3
Location: internal/backup.go:444
Claim: The extracted validateStartupConfig reads GIT_BACKUP_DIR with os.LookupEnv while displayStartupConfig and runProviderBackups read it with GetEnvOrFile, so the documented _FILE indirection is accepted in two places and rejected in the third.
Consequence: Set GIT_BACKUP_DIR_FILE=/run/secrets/dir and leave GIT_BACKUP_DIR unset. Startup logs 'root backup directory: <dir>' and then Run fails with 'environment variable GIT_BACKUP_DIR must be set'. Separately, the newline-trimmed value is what gets stat'ed and used for the working directory, while runProviderBackups re-reads the untrimmed value, so a GIT_BACKUP_DIR with a trailing newline backs up to a different path from the one validated.
Fix: —

### Item 4
Location: internal/backup.go:277
Claim: displayBitBucketStartupConfig is gated only on BITBUCKET_EMAIL, which disagrees with the bitbucketAPITokenDefined/bitbucketOAuthDefined helpers this PR adds for deciding whether BitBucket runs.
Consequence: A user on OAuth2 (BITBUCKET_USER/KEY/SECRET, no email) gets BitBucket backups but no 'BitBucket backups to keep / compare method / backup LFS' lines at startup. A user with BITBUCKET_EMAIL set and no API token sees the BitBucket config logged although BitBucket will not run. Gating on 'bitbucketAPITokenDefined() || bitbucketOAuthDefined()' fixes both. Sourcehut has no startup display at all.
Fix: —

### Item 5
Location: internal/backup.go:228
Claim: logProviderBackupLFS checks existence with GetEnvOrFile but evaluates the value with envTrue, which reads only os.Getenv, so the GetEnvOrFile call is redundant and the _FILE form can never log true.
Consequence: Set GITHUB_BACKUP_LFS_FILE pointing at a file containing 'true'. GetEnvOrFile reports it exists, envTrue(GITHUB_BACKUP_LFS) reads the empty env var and returns false, and nothing is logged, at the cost of a file read per provider. The same pattern is used for GITHUB_SKIP_USER_REPOS at line 240. Simpler equivalent: 'if envTrue(envVar)'.
Fix: —

### Item 6
Location: internal/backup.go:495
Claim: scheduleBackups creates a gocron scheduler unconditionally, including for the one-shot default path where it is never started or shut down.
Consequence: With neither GIT_BACKUP_INTERVAL nor GIT_BACKUP_CRON set, every Run() allocates a scheduler and its goroutines and then abandons them; the test suite calls Run() repeatedly in one process and accumulates them. The scheduler is also not shut down when NewJob fails, for example on an invalid cron expression. Now that scheduling is isolated, creation belongs inside runScheduledJob.
Fix: —

### Item 7
Location: internal/backup.go:105
Claim: The refactor leaves three independent encodings of 'is this provider configured' (the tokenProviders table, the display*StartupConfig guards, and checkProvidersDefined with enabledProviderAuth) instead of one provider table.
Consequence: The three already disagree: the run gate for Gitea is GITEA_TOKEN while validation accepts GITEA_APIURL alone; the BitBucket display gate is the email only; the Azure DevOps run gate is the username only and sits in a table named tokenProviders; Sourcehut is in the run table but has no display function. Adding or changing a provider means editing three places. One table of name, auth env vars, run function and display options would drive all three.
Fix: —

### Item 8
Location: internal/backup.go:205
Claim: The new shared logProviderOrgs helper keeps the misspelling 'Organistations' and a redundant strings.ToLower(orgs) != "" comparison.
Consequence: All three provider org log lines (GitHub, Gitea, Azure DevOps) now emit the typo from one place, so grepping logs for 'Organisations' or 'Organizations' finds nothing, and the helper's own doc comment spells it correctly. Lowercasing before an emptiness check is wasted work; 'orgs != ""' is equivalent.
Fix: —

### Item 9
Location: internal/backup_test.go:629
Claim: The rewritten TestGiteaOrgsRepositoryBackup loop calls resetBackups() in each switch case and again unconditionally after the switch.
Consequence: Each iteration clears the backup directory twice, and a deferred resetBackups runs as well. The per-case calls are dead weight and the switch can shrink to the two assertion calls. The extracted helpers also rebuild the path to the 'gitea.lessknown.co.uk' directory seven times rather than computing it once.
Fix: —

### Item 10
Location: docker/Dockerfile:8
Claim: The merged RUN keeps 'rm -f "/var/cache/apk/*"', whose quoted glob never expands, and the touched curl line still lacks --fail.
Consequence: The rm targets a literal file named '*' and removes nothing; it is also unnecessary with 'apk add --no-cache'. Without -f, a bad TAG makes curl save the 404 HTML page as soba.tar.gz, so the build fails later with a confusing tar error rather than an HTTP error. The release workflow sets TAG=latest on main, giving '.../releases/download/latest/...', which is not a valid release asset path.
Fix: —
