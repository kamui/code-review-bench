# Local evidence publication, 2026-10-08

The [evidence release](https://github.com/kamui/code-review-bench/releases/tag/evidence-local-bench-2026-10-08-v1) contains two immutable packages. The local capture is 221,648,255 bytes, about 211.4 MiB. It preserves 23,961 distinct payloads covering 34,029 original paths. Sixteen compressed mappings also identify 42,255 paths with identical bytes already in Git and 843 paths withheld from publication. The [capture summary](capture-summary.json) pins those mappings. The [manifest](../../../bench/evidence/manifests/local-capture-2026-10-08-v1.json) indexes each stored object by hash, size and original permission bits.

The user approved public disclosure of this prepared local-only package. It includes selected benchmark sessions, diagnostics, explicit configuration and experiment artifacts. Selection excluded credential files, unselected home/account state, installed client binaries, Git metadata, source clones and generated dependency/build/explorer copies. Credential signatures, known account values or unreviewed nested archives blocked 843 candidate paths. Some flags may be public test fixtures; they remain withheld rather than being silently redacted. The scan is a filter, not a guarantee about every possible secret format.

The [inventory summary](inventory-summary.json) covers the execution roots and 30 worktrees, including the nested `finish-it-7` worktree. The scan was read-only and non-atomic. Full path inventories remain in the migration workspace; they are not shared retirement inventories. [Selection totals](selection-summary.json) distinguish the excluded categories. None of those exclusions authorize deletion.

## Retrieve an original local file

From a checkout containing this PR's records, choose a root name from `capture-summary.json` and the original relative path:

```sh
python3 docs/research/evidence-migration-2026-10-08/restore-local.py \
  bench-runs '<run>/<attempt>/timing.json' /tmp/restored-timing.json
```

The helper fetches the root's mapping, resolves the stored object or pinned Git copy, verifies its original hash and size, and creates a new output file with the original permissions. It refuses an existing output, a withheld path or an unsafe output link. Git references retain commit `92aa9400` as historical provenance. When the current tree lacks those bytes, the helper fetches the matching hash from the `local-git-fallbacks-2026-10-08-v1` manifest. Restoration no longer needs that old commit, including after a history rewrite.

Selective retrieval materializes only the requested object, but the first network fetch downloads its containing 211.4 MiB package. The ignored cache amortizes later retrieval. The repository's normal `evidence_store.py fetch`, `verify` and `--offline` behavior applies. The manifest itself adds about 7.3 MiB of plain JSON to Git; the payload stays in the release.

## Verification

A real fresh clone from GitHub, at `30909ce83385169de68680b746ffe2ac6391fc44`, received only the new manifests, mapping summary and verification scripts. Bubblewrap hid `/home/jack/.t3`, including all originating runs, cache and worktrees. The pilot payloads were removed from this disposable checkout before starting, and both exercises started without an evidence cache.

The [pilot](../../../bench/evidence/manifests/local-pilot-2026-10-08-v1.json) contains four original archives: successful Codex review `2026-10-03-codex-ce-sol61-high-selected/att-001`, failed Claude review `2026-10-02-claude-builtin-selected/att-006`, its successful replacement `att-007`, and grading attempt `rubric-v2-2026-09-30/2026-09-26-x384-lifecycle/q-soba-195/attempt-1`. Its 35 members match their original bytes and permissions. The archives were already public in Git; source/public blob identities were checked before copying them to the release.

- [Pilot consumer receipt](../../../bench/evidence/receipts/local-pilot-2026-10-08-v1-consumers.json): selective fetch, collector, legacy import verification, explorer evidence downloads and offline retrieval passed. Frozen attempt and grading records stayed unchanged. Import verification retained nine historical expected-hash mismatches across 2,775 files and 288 available archives.
- [Capture consumer receipt](../../../bench/evidence/receipts/local-capture-2026-10-08-v1-consumers.json): every published object and mapping verified; original-path restoration passed for transcript, reproducibility, executable and Git-backed samples. Withheld paths and existing outputs were refused. Offline retrieval passed.
- Separate [pilot](../../../bench/evidence/receipts/local-pilot-2026-10-08-v1-restoration.json) and [capture](../../../bench/evidence/receipts/local-capture-2026-10-08-v1-restoration.json) restoration receipts record remote downloads into empty temporary directories.
- [Before](accounting-before.json) and [after](accounting-after.json) ledger results are identical for 21 controlling queues and 417 reservations. One outstanding reservation remains. Publication did not resolve it.

`verify-pilot.py` and `verify-capture.py` reproduce the cold checks after the stated payload/cache removal in a disposable checkout with the originating `.t3` directory hidden. The initial pilot run completed its evidence checks but could not record its Git commit because `/dev/null` was unavailable inside the sandbox. The completed rerun used `--dev /dev`.

## Remaining work

Part of #81. This publication does not certify any whole workspace as complete or closed. No production source file, cache or workspace was deleted; reclaimed bytes are **0**.

- Review the 843 withheld paths, including original dependency archives with credential-like fixtures or nested archives. Their original local bytes remain intact.
- Finish source-pin, ignored scratch and unknown-path reconciliation. Capture any remaining required evidence before considering retirement.
- Make controlling ledger dependencies portable, or retain their referenced workspaces. A priced reservation can still depend on its local dispatch receipt.
- Obtain a complete candidate inventory, investigation closure and verified retirement plan during an exclusive maintenance window. Publish those records before dry-run/apply.
- Preserve a second recovery copy outside the execution machine. These checks establish remote availability and integrity, not recovery from deletion of the release itself.

The preparation recipe uses `evidence_inventory.py` once per root, `select-local.py` to classify candidate paths, and `prepare-local.py` to snapshot, scan, deduplicate and package the selected immutable bytes. These scripts do not upload or delete source evidence. Authentication signatures and related account values are used only locally for exclusion. Existing archive bytes and frozen records were not rewritten.
