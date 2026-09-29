# Review blind-833bf4

### Item 1
Location: internal/backup.go:689
Claim: checkProvidersDefined intercepts both Bitbucket providers in its switch, so they never reach checkUserAndPasswordProvider and partially configured Bitbucket credentials are never reported (pre-existing, carried through the rewrite).
Consequence: GITHUB_TOKEN set plus BITBUCKET_EMAIL set but BITBUCKET_API_TOKEN missing: validation passes with count=1 from GitHub, collectProviderBackupResults skips Bitbucket because bitbucketAPITokenDefined() is false, and the run reports success with Bitbucket never backed up and no 'parameter is not defined' error, unlike Azure DevOps in the same state.
Fix: —

### Item 2
Location: internal/backup.go:360
Claim: checkJustTokenProvider counts every non-empty auth parameter, including GITEA_APIURL and SOURCEHUT_APIURL, so an API URL with no token counts as a defined provider and contradicts checkProvider's documented '0 or 1' return.
Consequence: Only GITEA_APIURL is set (no GITEA_TOKEN, no other provider): count=1 so 'no providers defined' is not raised, collectProviderBackupResults gates on GITEA_TOKEN and runs nothing, and a one-shot run logs 'all backups failed' but exits 0 because failed==0. With both URL and token set the function returns 2.
Fix: —

### Item 3
Location: internal/backup.go:444
Claim: validateStartupConfig reads GIT_BACKUP_DIR with os.LookupEnv while displayStartupConfig and runProviderBackups read it with GetEnvOrFile, so the _FILE form is accepted in two places and rejected in the third.
Consequence: GIT_BACKUP_DIR_FILE=/run/secrets/backup_dir with GIT_BACKUP_DIR unset: startup logs 'root backup directory: /backups', then Run returns 'environment variable GIT_BACKUP_DIR must be set'. The TrimSuffix(backupDIR, "\n") on line 456 suggests file-sourced values were intended to work.
Fix: —

### Item 4
Location: internal/backup.go:277
Claim: displayBitBucketStartupConfig is gated only on BITBUCKET_EMAIL, so OAuth-configured Bitbucket logs no startup config even though the PR adds bitbucketAPITokenDefined/bitbucketOAuthDefined, which the backup path uses.
Consequence: BITBUCKET_USER, BITBUCKET_KEY and BITBUCKET_SECRET set with BITBUCKET_COMPARE=refs and BITBUCKET_BACKUPS=5: the Bitbucket backup runs but none of the 'BitBucket backups to keep / compare method / backup LFS' lines are logged. Conversely BITBUCKET_EMAIL alone prints Bitbucket config for a provider that will not run.
Fix: —

### Item 5
Location: internal/backup.go:495
Claim: scheduleBackups builds a gocron scheduler before the switch, so the one-shot default path creates a scheduler it never starts or shuts down.
Consequence: With neither GIT_BACKUP_INTERVAL nor GIT_BACKUP_CRON set, every Run() allocates a scheduler and its background goroutine that is never released; the test suite calls Run() many times in one process and accumulates them. Creating the scheduler inside runScheduledJob would avoid this.
Fix: —

### Item 6
Location: internal/backup.go:475
Claim: The new createWorkingDir re-implements the working-directory resolution that resolveWorkingDir in the same file already provides.
Consequence: The GIT_WORKING_DIR-or-default rule now lives in two places (lines 140-146 and 475-478); a change to one, such as _FILE support or path cleaning, would make the directory created at startup differ from the one runProviderBackups uses and cleans up. Call resolveWorkingDir(backupDIR) instead.
Fix: —

### Item 7
Location: internal/backup.go:228
Claim: logProviderBackupLFS (and the GitHub skip-user-repos check on line 240) calls GetEnvOrFile for existence but decides with envTrue, which reads os.Getenv only, so the GetEnvOrFile call is redundant and the _FILE form is silently ignored.
Consequence: GITHUB_BACKUP_LFS_FILE pointing at a file containing 'true': GetEnvOrFile opens and reads the file on every startup, envTrue returns false because GITHUB_BACKUP_LFS is unset, nothing is logged and LFS is not backed up. Either drop the GetEnvOrFile call or make envTrue use it.
Fix: —

### Item 8
Location: internal/backup_test.go:629
Claim: TestGiteaOrgsRepositoryBackup calls resetBackups() inside each switch case and again unconditionally after the switch, so every iteration clears the backup directory twice.
Consequence: The refactor kept the redundant calls on lines 629 and 632 alongside line 635, costing a second directory wipe per iteration and obscuring which call is the real cleanup. Remove the per-case calls; the switch then holds only the two assertion helpers.
Fix: —

### Item 9
Location: internal/backup.go:205
Claim: The misspelling 'Organistations' is now centralised in logProviderOrgs, and logProviderBackupLFS is called with the label 'Gitlab' (line 273) while the other GitLab lines use 'GitLab'.
Consequence: Searching logs for 'Organisations' or 'Organizations' finds no GitHub, Gitea or Azure DevOps org lines, and searching for 'GitLab' misses the 'Gitlab backup LFS: true' line. The refactor reduced each to a one-word fix but kept both.
Fix: —

### Item 10
Location: docker/Dockerfile:8
Claim: The merged RUN keeps rm -f "/var/cache/apk/*", where the quoted glob is never expanded, so the command does nothing.
Consequence: The shell passes the literal path '/var/cache/apk/*' to rm, which matches no file and is silenced by -f, so the step is dead code; apk add --no-cache already leaves no cache. Remove it or unquote the glob.
Fix: —
