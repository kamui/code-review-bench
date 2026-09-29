# Backup runtime and provider configuration

## Scope and measurements

Reviewed the `internal/backup.go` changes in `main...review-head`, including provider selection, startup logging, credential validation, `Run`, and scheduler setup. The resulting file is 774 lines, below the skill's 1,000-line decomposition threshold. The diff extracts `collectProviderBackupResults`, two Bitbucket credential predicates, provider logging functions, two credential-validation helpers, and startup/scheduler helpers.

## Findings

No actionable findings. `collectProviderBackupResults` retains the previous provider order and credential gates; the Bitbucket alternatives remain ORed. The validation helpers preserve the previous blank-value handling and aggregate errors. `Run` keeps the prior operation order while naming startup validation, working-directory creation, and scheduling steps. `runScheduledJob` consolidates the identical registration/start/shutdown sequence for interval and cron schedules, with their distinct options kept at the call sites.

The new logging helpers factor repeated formatting while the provider entry points keep activation credentials and provider-specific settings visible. A registry-driven rewrite could remove some repeated calls, but it would need special fields and branches for GitHub's user-repository flag and GitLab's default access level. That model would shift the differences into data and generic dispatch without removing meaningful behavior or making the configuration easier to follow, so no code-judo rewrite is recommended.

## Verification

- `git diff --check main...review-head` completed without output.
- `go test ./internal/ -count=1` passed with the packet-prescribed offline module/cache settings (`ok github.com/jonhadfield/soba/internal 5.208s`). Provider-credential tests skipped where credentials were unavailable, as allowed by the packet.
- Docker is unavailable under the packet, so the Dockerfile was not built.
