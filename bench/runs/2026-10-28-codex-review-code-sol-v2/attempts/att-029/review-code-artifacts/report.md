# Review report

Status **Approved**; coverage `complete`. `render_review.py` generated this report from the validated composition. The summary body below is the one the payload carries.

**Approved (advisory)** — no findings.

Reviewed `136a485`. All four changed files and affected provider, environment, notification, scheduling and test paths reviewed. A fresh verifier upheld both safety premises. Package execution hit a sandbox socket restriction; changed live tests were traced statically.

<details>
<summary>Review details</summary>

**Intent:** Resolve the reported SonarQube findings by extracting helpers while preserving backup behavior and message text, consolidating notification titles, removing a duplicate test, and tidying the Dockerfile.

**Issue fit:** No originating issue was supplied, so issue alignment is unavailable. The supplied PR text and pinned commit message establish intent; the described source changes are present. Source: user-supplied spec; commit messages in the range.

**Reviewed:** Range `main...review-head`: `136a485` against merge-base `c77f548`.

**Checks:**

- All checks concern clean head 136a4850df9aec8cf8813b27ba1533d4a17bc642 unless stated otherwise. No source, fixture, generated input or dependency was changed. Base was explicitly supplied as local main; its SHA equals the merge-base. This is a local range review without forge access.
- Reviewer ran `GOMODCACHE=/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-cache/gomodcache GOCACHE=/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-cache/gocache GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local timeout 300 go test ./internal/ -count=1 -v` once from the clone root. Exit 1: `panic: httptest: failed to listen on a port: listen tcp6 [::1]:0: socket: operation not permitted`, in the unchanged mock GitHub fixture. This is unavailable execution evidence for that path, not a product assertion failure. Full output: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-work/review-private.rHUH7u/internal-test.log`.
- `TestGiteaOrgsRepositoryBackup` skipped for absent GITEA_TOKEN. Static trace: setup, environment snapshot, deferred resetBackups and restore order, Run call, specific-org case (one absent org, two entries and both prefixes), wildcard case (both orgs, one/two entries and all three prefixes), and resets match base. Removed GitLab test duplicates the retained test exactly.
- Supplied `go build`, `go vet`, `golangci-lint run` (0 issues), and full-suite pass claims are unused/unavailable because the PR text supplies no exact input identity or readable output. The focused package run compiled the changed code. The claimed 13 SonarQube resolutions cannot be independently counted without scanner evidence; no material decision depends on the count.
- Docker is unavailable, so the Dockerfile was inspected statically: identical package set, command order and download path; only command coalescing, package ordering and shell quoting change.
- Verification used one initial fresh-context gpt-6.1-sol/high worker, awaited with wait_agent. Both scoped scheduling and provider/cleanup preservation premises hold; no follow-up was required. These rulings do not claim stronger shutdown or filesystem guarantees than base.
- Repository tree identity remained d50974711ad3921de8e81016e044188ad8bd0358 with a clean worktree before and after execution. A concurrent store-selector read briefly failed while another selector wrote the store; sequential recovery consumed all four chunks. A verifier-input evidence range exceeded envfile.go length; corrected before bundle construction and dispatch. Neither incident left missing evidence.

</details>

<!-- review-run head=136a4850df9aec8cf8813b27ba1533d4a17bc642 base-ref=main base-sha=c77f548cbd340d2e744da7c6d92a54372b4b900b merge-base=c77f548cbd340d2e744da7c6d92a54372b4b900b workflow=v5b-30-x382 packet_context=none supplied_inputs=yes issues=none coverage=complete -->

## Run

Head, base, merge-base, workflow, packet identity, issues and coverage are in the run trailer above.

- Repository: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone`
- Target: range `main...review-head`
- Merged: no
- Supplied issues or specs: `review-packet-195`

## Requirements

- `spec-review-packet-195/pr-body/"decomposed into focused helpers"` (acceptance): met. internal/backup.go:97-137,190-294,326-540,680-717; internal/backup_test.go:600-681: named functions delegate to focused helpers.
- `spec-review-packet-195/pr-body/"Behaviour and log output are unchanged"` (acceptance): met. Base/head control-flow comparison in internal/backup.go:40-142,190-294,326-558,680-717; internal/notify.go:28-42,104,191,219. Predicates, provider order, log message text, errors, singleton options and notification status branches are preserved for stable configuration; verifier initial/accounting.json confirms the scheduling and cleanup premises. Source locations in logger prefixes naturally move with helper extraction.
- `spec-review-packet-195/pr-body/"backup status titles extracted into constants"` (acceptance): met. internal/notify.go:28-42,104,191,219: shared constants and backupStatusTitle used by Telegram, ntfy and Slack.
- `spec-review-packet-195/pr-body/"an exact duplicate"` (acceptance): met. Base internal/backup_test.go:551-591 has identical GitLab test bodies; head internal/backup_test.go:600-681 retains both Gitea organisation cases, directory counts, prefix assertions and cleanup order.
- ``spec-review-packet-195/pr-body/"merged consecutive `RUN` instructions"`` (acceptance): met. docker/Dockerfile:5-14: user creation and package installation share one RUN, identical package set is sorted, complete download URL including TAG is quoted.
- `spec-review-packet-195/pr-body/"Fixes all 13 open SonarQube findings"` (supporting): not-verifiable. No pinned SonarQube finding list or readable scanner result was supplied; source changes match the described remediation mechanisms, but the count and scanner outcome cannot be independently established. This supporting assertion adds no missing acceptance outcome.
- `spec-review-packet-195/pr-body/"pass locally"` (supporting): not-verifiable. PR-body local-check claim has no exact clean-tree identity, environment or readable output; not reusable. Reviewer package execution at the pinned clean head compiled successfully but stopped on a sandbox-denied localhost socket, with live provider cases skipped. No claim of a full pass; source traces settle the changed behavior.

## File coverage

- `docker/Dockerfile`: reviewed
- `internal/backup.go`: reviewed
- `internal/backup_test.go`: reviewed
- `internal/notify.go`: reviewed

## Check evidence

- `go build` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, PR body reports success without a pinned clean-tree identity or readable logs; no forge access. The claim is not accepted as execution evidence.
- `go vet` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, PR body reports success without a pinned clean-tree identity or readable logs; no forge access. The claim is not accepted as execution evidence.
- `golangci-lint run` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, PR body reports success without a pinned clean-tree identity or readable logs; no forge access. The claim is not accepted as execution evidence.
- `full test suite` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, PR body reports success without a pinned clean-tree identity or readable logs; no forge access. The claim is not accepted as execution evidence.
- `GOMODCACHE=/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-cache/gomodcache GOCACHE=/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-cache/gocache GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local timeout 300 go test ./internal/ -count=1 -v` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: reviewer-executed, Exit 1: environment failure, not a demonstrated assertion regression. internal-test.log:123-148 records httptest listener panic: socket: operation not permitted. Live provider tests, including changed TestGiteaOrgsRepositoryBackup, skip for missing credentials; earlier local validation tests pass. The changed test is settled by a base/head static trace.

## Verification

- Allowance: initial batch spent, follow-up unspent.
- Batch `initial` (initial), `spawn_agent(fork_turns=none, model=gpt-6.1-sol, reasoning_effort=high), awaited to completion with wait_agent`: bundle `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-work/review-private.rHUH7u/initial`, raw return `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-work/review-private.rHUH7u/initial-return.json`, accounting `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-work/review-private.rHUH7u/initial/accounting.json`.
- Safety premise `backup/scheduled-singleton`, area `concurrency`, batch `initial`: holds. For interval and cron runs, extracting runScheduledJob preserves singleton rescheduling, interval-only immediate start, assignment of the global job before scheduler start, and waiting for shutdown before returning. Evidence: internal/backup.go:387-512 @c77f548cbd340d2e744da7c6d92a54372b4b900b; internal/backup.go:397-558 @136a4850df9aec8cf8813b27ba1533d4a17bc642
- Safety premise `backup/provider-cleanup-order`, area `data-integrity`, batch `initial`: holds. With stable environment/file credential values, collectProviderBackupResults selects the same providers in the same serial order as base, and runProviderBackups retains its deferred cleanup after provider completion and notification, with the same working-directory and backup-directory arguments. Evidence: internal/backup.go:30-177 @c77f548cbd340d2e744da7c6d92a54372b4b900b; internal/backup.go:30-189 @136a4850df9aec8cf8813b27ba1533d4a17bc642; internal/constants.go:120-160 @136a4850df9aec8cf8813b27ba1533d4a17bc642; internal/envfile.go:10-59 @136a4850df9aec8cf8813b27ba1533d4a17bc642

## Routed

- Unresolved: none.
- Disputed: none.

## Artifacts

- record: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-work/review-private.rHUH7u/record.json`
- payload: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-work/review-private.rHUH7u/payload.json`
- batch: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-work/review-private.rHUH7u/batch.json`
- composition: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-work/review-private.rHUH7u/composition.json`
- private_dir: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-work/review-private.rHUH7u`
- store: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-work/review-private.rHUH7u/review-context.json`
- skill_root: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-029/clone-work/frozen-skill`
