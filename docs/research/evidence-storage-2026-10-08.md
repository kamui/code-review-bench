# Execution storage census, 2026-10-08

This read-only, non-atomic scan used `bench/tools/evidence_inventory.py` against the two execution roots and each of the 29 worktrees separately. The [aggregate inventory](evidence-storage-inventory-2026-10-08.json) records every root's allocated sizes and classification counts. It includes ignored files; it does not follow symlinks. The host was not quiesced, so this is planning evidence, not a retirement snapshot.

| Root | Allocated GiB |
| --- | ---: |
| `~/.t3/bench-runs` | 12.536 |
| `~/.t3/bench-cache` | 4.522 |
| 29 worktrees combined | 52.915 |

Ignored `.local` trees account for 30.542 GiB of those worktrees. The three largest are `t3code-711b0740`, `t3code-fce34e92` and `t3code-8d1674bf`. Counts include repeated copies and may double-count hard-linked blocks. They do not establish unique evidence size or recoverable space. The earlier issue's 2026-10-07 measurements used a different moment and measurement method; these are not a before/after performance or cleanup result.

Every encountered non-directory entry received a class and reason in the full census. Unknown files, symlinks and development changes remain blocked. The aggregate records the compressed full census's hash and size; that temporary host report is not represented as shared or durable evidence. Retirement requires a new candidate-specific complete inventory captured during maintenance, published alongside its closure and restoration receipt.

No production archive was uploaded, existing evidence moved or workspace retired for this census. Verified reclaimed bytes for this implementation: **0**. Current tracked archives remain available through Git.

## Rollout still required

- Review each candidate's local-only and unknown files, including ignored scratch, sessions and diagnostics. The census's classifications do not establish completeness.
- Inventory all controlling queues and inspect active attempts and outstanding reservations. Preserve zero-charge proofs that still depend on a workspace, even when marked resolved. This census does not assert that any queue is settled or candidate inactive.
- Choose and publish real successful-review, failed/replacement and grading packages. Preserve existing missing/mismatched statuses and original dependency archive identities.
- Push their manifests and per-candidate closure/restoration records; verify the consumers from a real fresh checkout without original host paths. The implementation's automated cold tests use fixtures and a fake asset service, not a production publication receipt.
- Close investigations explicitly, classify every unknown path, and run retirement dry-runs. Leave all candidates blocked until those checks pass; record actual free-space changes only after approved rollout removal.

See [the storage and retirement workflow](../evidence-storage.md). The remaining production migration belongs to issue #81; this tooling PR does not close that issue.
