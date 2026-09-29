# Review blind-95bb6f

### Item 1
Location: internal/backup.go:348
Claim: checkJustTokenProvider counts each non-empty auth parameter independently, so a Gitea or Sourcehut config with only the API URL set passes validation as a defined provider even though the token is missing.
Consequence: GIT_BACKUP_DIR and GITEA_APIURL set, GITEA_TOKEN unset: checkProvidersDefined gets count=1 and no error, Run proceeds, collectProviderBackupResults gates on GITEA_TOKEN and runs nothing, getBackupsStats returns 0/0, the log says 'all backups failed' but failed==0 so the one-shot process exits 0 having backed up nothing. The new doc comment also contradicts checkProvider's documented '0 or 1' return, since this helper returns 2 for a fully configured Gitea.
Fix: —

### Item 2
Location: internal/backup.go:277
Claim: displayBitBucketStartupConfig is gated only on BITBUCKET_EMAIL, so BitBucket OAuth2 users get no startup config output, although the PR adds bitbucketAPITokenDefined/bitbucketOAuthDefined helpers that express the right condition.
Consequence: BITBUCKET_USER, BITBUCKET_KEY and BITBUCKET_SECRET set with BITBUCKET_COMPARE=refs and BITBUCKET_BACKUPS=5, no BITBUCKET_EMAIL: the function returns early and logs no retention, compare method or LFS setting, while the backup itself runs via OAuth2. Conversely, BITBUCKET_EMAIL alone with no API token prints BitBucket config for a provider that will never run.
Fix: —

### Item 3
Location: internal/backup.go:689
Claim: checkProvidersDefined still bypasses checkProvider for both BitBucket methods, so partially configured BitBucket credentials are never reported and the BitBucket entries in userAndPasswordProviders are dead on this path.
Consequence: GITHUB_TOKEN set plus BITBUCKET_EMAIL set but BITBUCKET_API_TOKEN missing: validation returns nil with no 'parameter is not defined' error, unlike the equivalent Azure DevOps case (username without PAT). BitBucket is silently skipped on every run and the user believes it is being backed up.
Fix: —

### Item 4
Location: internal/backup.go:444
Claim: validateStartupConfig reads GIT_BACKUP_DIR with os.LookupEnv while displayStartupConfig and runProviderBackups use GetEnvOrFile, so the _FILE form is displayed but then rejected, and the newline-trimmed value validated here differs from the untrimmed one used for the backup.
Consequence: Only GIT_BACKUP_DIR_FILE is set: startup logs 'root backup directory: /backups' and then Run fails with 'environment variable GIT_BACKUP_DIR must be set'. With GIT_BACKUP_DIR='/backups\n', Stat and createWorkingDir use '/backups' but runProviderBackups and cleanupWorkingDir use the untrimmed path.
Fix: —

### Item 5
Location: docker/Dockerfile:13
Claim: The rewritten download line still runs curl without --fail, and the release workflow passes TAG=latest on main, which does not match GitHub's releases/latest/download URL form.
Consequence: A build on main with TAG=latest (or TAG unset, giving 'download//') requests releases/download/latest/soba_linux_amd64.tar.gz and gets a 404; curl saves the HTML body as soba.tar.gz and exits 0, and the failure only surfaces as a confusing 'tar: invalid magic' error in the next step. This assumes no release is literally tagged 'latest', which I could not check offline.
Fix: —

### Item 6
Location: internal/backup.go:495
Claim: scheduleBackups creates a gocron scheduler unconditionally before the switch, so one-shot mode and the NewJob error path both leave a started scheduler goroutine that is never shut down.
Consequence: With neither GIT_BACKUP_INTERVAL nor GIT_BACKUP_CRON set, every Run() call leaks the goroutine NewScheduler spawns; tests that call Run() repeatedly accumulate them. An invalid cron expression returns 'failed to create job' without calling s.Shutdown(). Creating the scheduler inside runScheduledJob and shutting it down on error would fix both.
Fix: —

### Item 7
Location: internal/backup_test.go:629
Claim: resetBackups() is called inside each switch case and again unconditionally after the switch, so the backup directory is wiped twice per iteration, and the switch on the loop variable is itself redundant.
Consequence: Each iteration of TestGiteaOrgsRepositoryBackup does two removeContents passes over the backup directory. A table of {org, assert func} pairs with one resetBackups call would remove the switch and the duplicate calls.
Fix: —

### Item 8
Location: internal/backup.go:228
Claim: logProviderBackupLFS (and the GitHub skip-user-repos check at line 240) calls GetEnvOrFile only to discard the value and then decides via envTrue, which reads os.Getenv only, so the first call is wasted work and the _FILE form is never honoured.
Consequence: GITHUB_BACKUP_LFS_FILE pointing at a file containing 'true': GetEnvOrFile opens and reads the file and returns exists=true, envTrue returns false because the plain variable is unset, and nothing is logged. The file read happens at every startup for every provider for no effect; `if envTrue(envVar)` is equivalent and cheaper.
Fix: —

### Item 9
Location: internal/backup.go:205
Claim: The shared logProviderOrgs helper centralises the misspelt 'Organistations' log text, and displayGitLabStartupConfig passes the label 'Gitlab' to the LFS helper while using 'GitLab' for every other line.
Consequence: Operators grepping logs for 'Organisations', 'Organizations' or 'GitLab backup LFS' find nothing. The typo previously existed in three separate literals and is now a one-line fix the refactor did not take; line 273 has the inconsistent label.
Fix: —

### Item 10
Location: internal/backup.go:101
Claim: The new bitbucketAPITokenDefined/bitbucketOAuthDefined helpers are not reused by Bitbucket() in bitbucket.go, which repeats the same five GetEnvOrFile reads and the same completeness expressions immediately after the caller evaluated them.
Consequence: Every backup run reads the BitBucket credentials twice (env or secret files), once in collectProviderBackupResults and again at bitbucket.go:15-25, and the completeness rule now lives in two places that can drift apart. If they drift, the 'authentication not properly configured' branch in Bitbucket() becomes reachable or the gate becomes wrong.
Fix: —
