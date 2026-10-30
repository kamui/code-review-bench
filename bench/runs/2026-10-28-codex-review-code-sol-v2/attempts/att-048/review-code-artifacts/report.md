# Review report

Status **Approved**; coverage `complete`. `render_review.py` generated this report from the validated composition. The summary body below is the one the payload carries.

**Approved (advisory)** — no findings.

Reviewed `136a485`. All four changed files and affected credential, scheduling, notification, test and Docker release paths reviewed. Fresh verifier confirmed two scoped safety premises; execution limits are recorded below.

<details>
<summary>Review details</summary>

**Intent:** Resolve the listed SonarQube findings through helper extraction, title deduplication, duplicate-test removal and Dockerfile cleanup while preserving backup behavior.

**Issue fit:** No originating issue was supplied, so issue alignment is unavailable. The supplied PR packet and commit-136a485 establish the reviewed intent. Source: user-supplied spec; commit messages in the range.

**Reviewed:** Range `main...review-head`: `136a485` against merge-base `c77f548`.

**Checks:**

- Clean head 136a4850df9aec8cf8813b27ba1533d4a17bc642; no source, fixture, dependency or configuration changes during review. One bounded package test attempt used offline caches and Go 1.26.5.
- Reviewer command `go test ./internal/ -count=1 -v -timeout=240s` exited 1: the unchanged GitHub fixture panicked at socket creation (`operation not permitted`), before invoking backup code. This is unavailable runtime evidence, not an introduced assertion failure. Full output: /home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-048/clone-work/review-code.FPTB3z/go-test-internal.log.
- Live provider tests, including `TestGiteaOrgsRepositoryBackup`, skipped without credentials. The changed Gitea test was traced from setup through Run, directory assertions, prefix checks and cleanup in both org cases; all original assertions remain.
- Author-reported `go build`, `go vet`, `golangci-lint run` and full-suite passes were not reused: exact tested inputs and raw logs were not supplied. SonarQube inventory/results are unavailable offline.
- Docker unavailable; Dockerfile command chain, package set and URL quoting checked statically against base and .github/workflows/release.yml.
- Fresh verifier returned holds for scheduler option/task/start ordering and stable-credential provider selection/serial execution/deferred cleanup. Evidence reconciled against pinned sources in initial-v2/accounting.json.

</details>

<!-- review-run head=136a4850df9aec8cf8813b27ba1533d4a17bc642 base-ref=main base-sha=c77f548cbd340d2e744da7c6d92a54372b4b900b merge-base=c77f548cbd340d2e744da7c6d92a54372b4b900b workflow=v5b-30-x382 packet_context=none supplied_inputs=yes issues=none coverage=complete -->

## Run

Head, base, merge-base, workflow, packet identity, issues and coverage are in the run trailer above.

- Repository: `jonhadfield/soba`
- Target: range `main...review-head`
- Merged: no
- Supplied issues or specs: `review-packet-195`

## Requirements

- `spec-review-packet-195/pr-body/"decomposed into focused helpers"` (acceptance): met. internal/backup.go:66,97-138,196-294,331-393,408-542,685-701; internal/backup_test.go:628-679.
- `spec-review-packet-195/pr-body/"Behaviour and log output are unchanged"` (acceptance): met. Complete base/head comparison of internal/backup.go:40-142,190-296,322-545,680-717; initial-v2/accounting.json premises backup/scheduler-extraction and backup/provider-dispatch-cleanup both hold under their stated conditions. Startup labels and messages match the removed branches; provider credential validation keeps the same error/count predicates.
- `spec-review-packet-195/pr-body/"backup status titles extracted into constants"` (acceptance): met. internal/notify.go:27-41,104,194,218; all three original switches use the same predicates, Unicode string literals and default branch as the helper.
- `spec-review-packet-195/pr-body/"an exact duplicate"` (acceptance): met. internal/backup_test.go:551-568 matches the removed TestPublicGitLabRepositoryBackup2 body at base:570-587; Gitea setup, Run call, single-org exclusions, wildcard inclusion, directory counts, all three name-prefix checks and reset ordering preserved at head:600-679.
- ``spec-review-packet-195/pr-body/"merged consecutive `RUN` instructions"`` (acceptance): met. docker/Dockerfile:5-8,13; same package set, sequential && commands and quoted complete download URL.
- `spec-review-packet-195/pr-body/"all 13 open SonarQube findings"` (supporting): not-verifiable. The packet supplies no SonarQube issue inventory, run identity or readable analyzer output. The listed transformations are present; exact analyzer count cannot be independently checked offline and is not needed to settle a concrete defect.
- `spec-review-packet-195/pr-body/"full test suite pass locally"` (supporting): not-verifiable. Supplied summary lacks exact-head/environment metadata and raw output. Reviewer package test compiled but exited 1 on forbidden socket creation; live provider tests skipped, including the changed Gitea test. No passing full-suite claim is made.

## File coverage

- `docker/Dockerfile`: reviewed
- `internal/backup.go`: reviewed
- `internal/backup_test.go`: reviewed
- `internal/notify.go`: reviewed

## Check evidence

- `go build` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, Author-reported pass in supplied packet; exact tested tree, environment and raw output unavailable; not reused.
- `go vet` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, Author-reported pass in supplied packet; exact tested tree, environment and raw output unavailable; not reused.
- `golangci-lint run` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, Author-reported pass in supplied packet; exact tested tree, environment and raw output unavailable; not reused.
- `full test suite` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, Author-reported pass in supplied packet; exact tested tree, environment and raw output unavailable; not reused.
- `go test ./internal/ -count=1 -v -timeout=240s` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: reviewer-executed, Environmental interruption: TestGithubRepositoryBackupWithMockAPI panics before the backup call because httptest cannot create a socket (operation not permitted). Live tests skipped; changed Gitea test skipped without GITEA_TOKEN. Reviewed static traces settle affected paths.
- `Dockerfile build` at `136a4850df9aec8cf8813b27ba1533d4a17bc642`: unavailable, Docker unavailable by execution policy. Complete Dockerfile diff and release workflow statically inspected; command ordering and package set preserved.

## Verification

- Allowance: initial batch spent, follow-up unspent.
- Batch `initial` (initial), `collaboration.spawn_agent fork_turns=none model=gpt-6.1-sol reasoning_effort=high; awaited completed return with collaboration.wait_agent`: bundle `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-048/clone-work/review-code.FPTB3z/initial-v2`, raw return `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-048/clone-work/review-code.FPTB3z/initial-return.json`, accounting `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-048/clone-work/review-code.FPTB3z/initial-v2/accounting.json`.
- Safety premise `backup/scheduler-extraction`, area `concurrency`, batch `initial`: holds. For interval and cron scheduling, the extraction preserves the singleton options and task, assigns the package job before scheduler Start, and calls the same waitForShutdown after Start as the base. Evidence: internal/backup.go:435-510 @main; internal/backup.go:492-563 @review-head
- Safety premise `backup/provider-dispatch-cleanup`, area `data-integrity`, batch `initial`: holds. With stable credential values, collectProviderBackupResults preserves the provider eligibility and serial order, while runProviderBackups still defers the same cleanup until after all provider calls and notifications return, including provider-error results. Evidence: internal/backup.go:30-187 @main; internal/backup.go:30-187 @review-head; internal/envfile.go:10-59 @review-head; internal/bitbucket.go:1-88 @review-head; internal/gitea.go:1-55 @review-head

## Routed

- Unresolved: none.
- Disputed: none.

## Artifacts

- record: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-048/clone-work/review-code.FPTB3z/record.json`
- payload: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-048/clone-work/review-code.FPTB3z/payload.json`
- batch: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-048/clone-work/review-code.FPTB3z/batch.json`
- composition: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-048/clone-work/review-code.FPTB3z/composition.json`
- private_dir: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-048/clone-work/review-code.FPTB3z`
- store: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-048/clone-work/review-code.FPTB3z/review-context.json`
- skill_root: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-048/clone-work/frozen-skill`
- intent_spec: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-048/clone-work/review-code.FPTB3z/spec-review-packet-195.md`
- test_output: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-048/clone-work/review-code.FPTB3z/go-test-internal.log`
- commit_description: `/home/jack/rq-sol/2026-10-28-codex-review-code-sol-v2/att-048/clone-work/review-code.FPTB3z/commit-136a485`
