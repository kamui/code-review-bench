1. **[P2] The saved-record test bypasses the compatibility alias** — [test_ruling_record.py:423](/home/jack/.t3/worktrees/code-review-bench/pre-existing-rename/bench/tools/test_ruling_record.py:423).

   `SavedRecordsTest` still calls `ruling_record.load`, then validates the original decision type. When the earlier activation records land, this test rejects `older-admission` and `older-grouping`, despite `read_before` supporting them. I ran the existing test against the activation worktree, rooting validation there: it fails on `01-kubernetes-truncated-event-note.OP-candidate-x-1.before.json`. That record validates through `read_before`. Change this reader to use the alias; the new synthetic test does not cover this consumer.

2. **[P2] The documented activation grader produces rejected v3 keys** — [pre_existing_grading.py:6](/home/jack/.t3/worktrees/code-review-bench/pre-existing-rename/bench/tools/pre_existing_grading.py:6), [pre-existing-problems.md:38](/home/jack/.t3/worktrees/code-review-bench/pre-existing-rename/docs/pre-existing-problems.md:38).

   The activation guide still points to the pinned grader that mandates `older_problems`, `older_candidate`, and `older-problem`. The validator now requires their renamed forms under the same `current-verdicts/v3` contract. I passed the pinned grader’s JSON example through validation: it fails with “v3 claims need the v2 fields plus pre_existing_problems and pre_existing_candidate”; renaming those two keys makes it pass. Preserve the pinned proposal, but require a compatible versioned grader/policy before activation.

Checks found sound:

- Current records and all nine current schema examples validate; current records contain no renamed fields or enum values.
- All **198** current run/target fingerprints match `origin/main`.
- The alias distinguishes opening path and hash, preserves source data, and validates the earlier records through updated production readers.
- Validator copy lists and all 23 `runner_files` entries resolve. Import verification passes.
- Excluded paths are untouched; I found no active grading or scoring behavior change.

Verification: `current_grading.py check`, import checking, `bun run typecheck`, and diff checking passed. `bun test ./src`: **64 passed**, four blocked by read-only temporary storage. Focused Python unittests: **30 passed**, 197 blocked by unavailable temporary storage. No files changed.