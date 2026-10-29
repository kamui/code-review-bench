# Review report

Status **Approved**; coverage `complete`. `render_review.py` generated this report from the validated composition. The summary body below is the one the payload carries.

**Approved (advisory)** — no findings.

Reviewed `136a485`. All four changed files and the complete merge-base diff reviewed, with provider wrappers, environment-file lookup, statistics, scheduler flow and changed-test setup/assertions/cleanup traced. Both safety premises independently verified; no actionable defects found. Clone remained clean at the pinned head.

<details>
<summary>Review details</summary>

**Intent:** Resolve SonarQube findings through helper extraction, shared notification titles, duplicate-test removal and Dockerfile cleanup while preserving backup behavior.

**Issue fit:** Issue alignment unavailable: no originating issue was supplied. Intent comes from the supplied PR body (spec-review-packet.md) and pinned commit-136a485. Source: user-supplied spec; commit messages in the range.

**Reviewed:** Range `main...review-head`: `136a485` against merge-base `c77f548`.

**Checks:**

- Exact reviewed head 136a4850df9aec8cf8813b27ba1533d4a17bc642, clean tree; local base main explicitly supplied, merge-base c77f548cbd340d2e744da7c6d92a54372b4b900b. Review is a local committed-range audit; no forge access or publication occurred.
- Reviewer executed from clone root: `GOMODCACHE=/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-cache/gomodcache GOCACHE=/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-cache/gocache GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local timeout 300 go test ./internal/ -count=1 -v` (Go 1.26.5), exit 1. [Raw output](/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-work/review-private.j3JW85/internal-test.log): ordinary validation/encryption tests passed before TestGithubRepositoryBackupWithMockAPI failed during fixture setup: `httptest: failed to listen on a port: listen tcp6 [::1]:0: socket: operation not permitted` (lines 123-124). This is an environmental failure, not an introduced assertion failure; remaining tests did not execute. No suite rerun.
- TestGiteaOrgsRepositoryBackup and other credential-dependent backups skipped (internal-test.log:58-60). Static comparison covers its guard, environment snapshot, preflight, Run call, per-org assertions, resets and deferred cleanup; assertions retain the same observation power. Live behavior was not executed.
- Author supplied `go build`, `go vet`, `golangci-lint run` (0 issues), full-suite pass and all-13-Sonar-findings claims in the PR body, associated with the reviewed change but without raw output, exact runner inputs or clean-tree metadata. All are unavailable for reuse; no independent build/vet/lint/Sonar results are claimed.
- Docker unavailable: image build not executed; Dockerfile reviewed statically for command chaining, unchanged package set, quoting, user and entrypoint.
- Initial fresh-context verifier (gpt-6.1-sol, high) returned holds for scheduler ordering/singleton preservation and sequential provider eligibility/cleanup preservation. Findings and questions are empty. The scheduling ruling preserves existing Shutdown/error handling and does not assert unconditional completion after a scheduler error.

</details>

<!-- review-run head=136a4850df9aec8cf8813b27ba1533d4a17bc642 base-ref=main base-sha=c77f548cbd340d2e744da7c6d92a54372b4b900b merge-base=c77f548cbd340d2e744da7c6d92a54372b4b900b workflow=v5b-30-x382 packet_context=none supplied_inputs=yes issues=none coverage=complete -->

## Run

Head, base, merge-base, workflow, packet identity, issues and coverage are in the run trailer above.

- Repository: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone`
- Target: range `main...review-head`
- Merged: no
- Supplied issues or specs: `spec-review-packet/pr-body`

## Requirements

- `commit-136a485/"behaviour and log output unchanged"` (acceptance): met. internal/backup.go:40-142,190-294,326-559,680-717 compared with base; provider eligibility/order, Run validation/error ordering, scheduling options and emitted message text preserved for stable configuration. Independent initial verification holds both safety premises. Logger source-line metadata necessarily moves with extracted helpers.
- `commit-136a485/"shared backupStatusTitle helper"` (acceptance): met. internal/notify.go:27-40,103,195,216; original title strings and three-branch selection preserved for Telegram, ntfy and Slack.
- `commit-136a485/"identical duplicate"` (acceptance): met. Base internal/backup_test.go:551-589 has identical GitLab test bodies; head retains TestPublicGitLabRepositoryBackup at 551-568. Gitea setup and cleanup at 600-640 and helpers at 644-681 retain directory presence/absence, counts and all repository-prefix checks from base 619-710.
- `commit-136a485/"quote the TAG variable"` (acceptance): met. docker/Dockerfile:5-13; same package set sorted, user creation joined to installation, complete download URL quoted; remaining image steps preserved.
- `spec-review-packet/pr-body` (supporting): not-verifiable. spec-review-packet.md supplies only check summaries with no raw outputs or runner identity; no Sonar results or Docker runner available. Reviewer internal tests compiled, then stopped at blocked httptest listener (internal-test.log:123-124); live tests skipped. Exact scanner count and author full-suite/lint/build claims are not established, with no independent correctness defect implied.

## File coverage

- `docker/Dockerfile`: reviewed
- `internal/backup.go`: reviewed
- `internal/backup_test.go`: reviewed
- `internal/notify.go`: reviewed

## Check evidence

- `GOMODCACHE=/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-cache/gomodcache GOCACHE=/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-cache/gocache GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local timeout 300 go test ./internal/ -count=1 -v` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: reviewer-executed, Exit 1: local httptest socket creation prohibited; setup failure, not product assertion. Credential-dependent cases skipped. Raw log: /home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-work/review-private.j3JW85/internal-test.log
- `go build` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, Author summary in spec-review-packet.md only; no raw output or exact input/runner metadata, so not reused.
- `go vet` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, Author summary in spec-review-packet.md only; no raw output or exact input/runner metadata, so not reused.
- `golangci-lint run (0 issues)` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, Author summary in spec-review-packet.md only; no raw output or exact input/runner metadata, so not reused.
- `full test suite` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, Author summary in spec-review-packet.md only; no raw output or exact input/runner metadata, so not reused.
- `SonarQube (13 findings resolved)` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, Author summary in spec-review-packet.md only; no raw output or exact input/runner metadata, so not reused.
- `docker build` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, Docker unavailable under execution policy; commands and final image configuration traced statically.

## Verification

- Allowance: initial batch spent, follow-up unspent.
- Batch `initial` (initial), `collaboration.spawn_agent(fork_turns=none, model=gpt-6.1-sol, reasoning_effort=high), awaited with collaboration.wait_agent until final return`: bundle `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-work/review-private.j3JW85/initial-built`, raw return `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-work/review-private.j3JW85/initial-return.json`, accounting `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-work/review-private.j3JW85/initial-built/accounting.json`.
- Safety premise `backup/scheduler-order-preserved`, area `concurrency`, batch `initial`: holds. For a successful interval or cron schedule, the refactor retains singleton rescheduling, assigns the package job before starting the scheduler, and waits for graceful shutdown; NewJob errors return before Start. Evidence: internal/backup.go:431-510@main; internal/backup.go:491-559@review-head; internal/backup.go:30-38@review-head
- Safety premise `backup/provider-cleanup-preserved`, area `data-integrity`, batch `initial`: holds. With stable environment and credential files, the extracted collection runs the same eligible providers once in the same sequential order with the same backup directory, and retains the deferred cleanup after collection, statistics and notification. Evidence: internal/backup.go:40-178@main; internal/backup.go:40-189@review-head; internal/envfile.go:10-68@review-head

## Routed

- Unresolved: none.
- Disputed: none.

## Artifacts

- record: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-work/review-private.j3JW85/record.json`
- payload: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-work/review-private.j3JW85/payload.json`
- batch: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-work/review-private.j3JW85/batch.json`
- composition: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-work/review-private.j3JW85/composition.json`
- private_dir: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-work/review-private.j3JW85`
- store: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-work/review-private.j3JW85/review-context.json`
- skill_root: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-012/clone-work/frozen-skill`
