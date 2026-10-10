# Shared benchmark evidence

Keep the evidence needed to inspect a published observation for as long as the repository publishes or relies on that observation. There is no age-based deletion of evidence. Local execution storage has a different lifetime: it can be retired after complete capture, shared publication, cold restoration and investigation closure. A superseded or failed observation still belongs to the record.

## Evidence contract v1

Small records stay in Git. Larger payloads live as immutable, content-addressed assets on this repository's GitHub Releases; their manifests stay in `bench/evidence/manifests/`. A clone discovers every package there, including its canonical repository. Forks use that repository, not their own releases. Historical tracked archives continue to work; the [repository payload migration](research/repository-storage-2026-10-08/README.md) moves their current copies to release storage.

| Category | Preserve |
| --- | --- |
| Inputs | Frozen runs, arms, targets, task packets, actual prompts, source revisions/diffs, exact skill and runner runtime, client versions, model/effort, explicit configuration, execution policy and deviations. Capture source bytes when the pinned revision cannot be retrieved independently. |
| Execution | Original reviews, normalized findings, complete parent/child transcripts, timing/dispatch records, context/isolation/audit receipts and source-tree identity checks. |
| Accounting and lineage | Requests, usage, rates, billing declarations, charges, reservations/resolutions, dispositions, failed predecessors and replacement links. Unknown usage stays unknown. |
| Grading | Inputs, rubrics, references, identity mappings, grader transcripts, verdicts, validation receipts, claims, rulings, mapping versions and supporting probes. |
| Unique artifacts | Diagnostics, reports, scripts, patches and changed/new source that explain the observation. Keep native-artifact indexes, archives and permissions. Classify unindexed scratch explicitly. |
| Integrity | Logical paths, ownership, byte sizes, hashes, original limitations, manifests and publication/restoration/retirement receipts. |

Use `file_attempt.py`, `regrade.py:archive_attempt`, `native_artifacts.py` and `tools/collect_run.py` first. Package their existing archives without recompressing or editing the archive bytes. The outer storage package preserves each inner file's SHA-256. Keep relevant `home` sessions and explicit configuration, including `home/.codex/config.toml`, and unique `clone-work` contents. Exclude credentials and unrelated account state. The packer rejects the known `auth.json` and `.credentials.json` filenames, but selection review must also check custom credential locations and archive contents.

Do not change frozen `attempt.json` paths, hashes or dispositions to migrate storage. Add mappings instead. Historical missing evidence stays missing; a mismatch retains both expected and available hashes. A replacement run cannot supply the original observation's missing evidence.

A record can name its transcript archive by a home-relative or absolute path, which only the originating checkout resolves. `tools/collect_run.py --run bench/runs/<run>` maps each attempt and probe record of a run in `bench/runs/<run>/transcripts.json`: the record, the archive's repository path, the frozen hash and `verified`. The repository path is the one the record holds, or the one it names under `artifacts/transcripts/` in its originating checkout. When neither the checkout nor release storage has that path and release storage holds exactly one archive with the frozen hash, the collector maps the record to the stored path. A record that names any other path gets `artifacts/transcripts/<run>/`, and the collector copies the archive there. The collector reads the archive from this checkout or from release storage and refuses a missing one or a changed hash. `file_attempt.py` records an archive filed inside the checkout that holds the record by its repository path, so a new record resolves in any checkout. The [2026-10-10 inventory](research/portable-evidence-2026-10-10/README.md) lists the references mapped this way.

When no copy holds a record's archive, `--missing <record>` writes its entry with the frozen hash and status `missing`. Later collections keep that entry until the archive verifies. The collector refuses every other archive it cannot verify, and consumers read `verified` entries only.

`bench/schema/evidence-storage.schema.json` defines the storage manifest. The tool validates it before publication or retrieval, then checks additional constraints: unique paths/identities, safe relative paths, ordinary files, permission bits and package limits. `subjects` names the covered attempts/batches; each package indexes its members and pins release/asset IDs, size and SHA-256. `publication.verified_at` records a successful remote download and restoration, not merely an upload response.

## Inventory before migration

Run from outside the storage being inspected, with a new output file:

```sh
python3 bench/tools/evidence_inventory.py \
  --root "$HOME/.t3/bench-runs" --root "$HOME/.t3/bench-cache" \
  --out /tmp/bench-storage-inventory.json
python3 bench/tools/evidence_inventory.py \
  --root /absolute/path/to/a/worktree --out /tmp/worktree-inventory.json
```

Inventory each worktree separately so its index can distinguish tracked copies from development changes. Every non-directory entry is listed, including ignored files, special files and symlinks. Nothing follows symlinks. Classifications are advisory: tracked-identical, local-evidence, reproducibility, rebuildable, development, account-state or unknown. A rebuildable label does not authorize removal. Allocated sizes include repeated copies and hard links; they are not unique evidence sizes or reclaim estimates. The scan is read-only, not an atomic snapshot of a running host. Quiesce a retirement candidate and repeat with `--hash-all` before its closure.

Inspect records for active attempts, unresolved accounting, missing/mismatched exports and ongoing investigations. Save that assessment alongside the inventory. Inventory covers only the supplied roots; enumerate every controlling queue, including ignored `.local` queues in other worktrees. The [initial host census](research/evidence-storage-2026-10-08.md) records rollout gaps.

## Package, publish and verify

Before publishing, write four facts in the batch's record: the exact destination (repository and release tag, or site), its visibility, the categories of the table above that the selection covers, and the owner's authorization with its date. An earlier authorization still applies when it names the same destination, visibility and categories. Sending evidence to an external service and publishing it on public GitHub are separate scopes, and authorization for one does not cover the other.

Create a selection outside frozen runs. `source` is a local regular file; `path` is the portable destination under `bench/`, `artifacts/` or `docs/`. Select evidence by subject rather than uploading an entire home or worktree.

```json
{
  "repository": "kamui/code-review-bench",
  "tag": "evidence-example-v1",
  "subjects": ["example/att-001"],
  "files": [
    {
      "source": "/absolute/export/att-001.tar.gz",
      "path": "artifacts/transcripts/example/att-001.tar.gz",
      "kind": "evidence"
    }
  ]
}
```

```sh
python3 bench/tools/evidence_store.py pack --selection /tmp/selection.json --out /tmp/evidence-package-v1
# Create a published evidence release once, before uploading to it.
gh release create evidence-example-v1 --repo kamui/code-review-bench \
  --title 'Benchmark evidence example v1' --notes 'Immutable benchmark evidence payloads.'
python3 bench/tools/evidence_store.py publish --manifest /tmp/evidence-package-v1/manifest.json \
  --out bench/evidence/manifests/example-v1.json
python3 bench/tools/evidence_store.py verify --manifest bench/evidence/manifests/example-v1.json \
  --receipt bench/evidence/receipts/example-v1-restoration.json
```

`publish` uploads without clobbering. It downloads each asset into an empty temporary directory, verifies its identity/hash and restores all indexed members before writing the published manifest. A retry verifies existing bytes instead of replacing them. An ambiguous upload triggers a read of the release, not another blind upload. A failed run may leave an uploaded asset, but never a successful publication receipt for unverified bytes.

Packages default to at most 1 GiB of uncompressed member bytes. `pack --part-bytes N` can raise that limit below 2 GiB; the compressed asset must also remain below 2 GiB. A single input larger than the selected part size is refused, not silently split or rewritten. Such inputs need an explicit versioned export/chunking design before migration. GitHub permits at most 1,000 assets per release; use another versioned release when full. See [GitHub release limits](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases#storage-and-bandwidth-quotas).

Commit and push the manifest, restoration receipt, mappings and small records. Only then can local retirement be considered. Publication does not settle accounting, close an investigation or establish scientific completeness: the maintainer must review the evidence contract for each subject.

## Retrieve from another checkout

Install Python 3.11+ and GitHub CLI, then authenticate `gh` for API downloads. `bun run evidence:fetch` retrieves the repository payload manifest before direct Python consumer commands. Development, build and verification package commands do this automatically. Inspect `bench/evidence/manifests/*.json`, then select a logical path or omit `--path` to fetch the whole manifest:

```sh
python3 bench/tools/evidence_store.py fetch --manifest bench/evidence/manifests/example-v1.json \
  --path artifacts/transcripts/example/att-001.tar.gz
python3 tools/import_benchmark.py --check
python3 tools/export_explorer.py
```

Downloads enter ignored `.cache/evidence/`. The tool verifies archive and member hashes, rejects unsafe members/symlinks, restores permissions and materializes the original layout. Changed files are never overwritten. `--offline` uses already verified local files/cache; a missing or corrupt item fails with a reason. `verify` always reads the remote assets into empty temporary directories, independently of this cache. Missing/deleted/replaced assets fail identity/hash checks.

The collector, import verifier, explorer evidence exporter and review cleanup resolver fetch missing mapped files through these manifests. Explorer publishes its existing evidence links after materialization. For native artifacts, fetch the indexed archive first, then use `native_artifacts.py verify` and `restore` as usual. Grading archives retain their existing `evidence.json` hashes. The cold-consumer test exercises successful, failed/replacement and grading fixtures after removing their originating storage, including executable native scratch restoration.

After a verified migration, remove large tracked payloads through an ordinary reviewed commit and add precise ignore entries for their materialized paths. Keep their mappings/manifests. This reduces future checkout size, not old Git history; no history rewrite or LFS migration is part of this workflow. The repository payload migration follows this procedure for the current archives, run stdout logs and upstream research snapshots.

The `.cache/evidence` directory can be removed when no fetch/consumer is using it. Doing so loses offline retrieval for unmaterialized files. Keep a separately retained copy of published packages for disaster recovery.

## Frozen definition commits

Packet selection reads a run's manifest at its `freeze_commit`, so that commit must stay retrievable from a ref the canonical repository keeps: a branch, a tag or a pull request head. A squash merge leaves the commit off `main`. GitHub then keeps it under `refs/pull/<number>/head` after the branch is deleted, and a default clone does not fetch that ref:

```sh
git fetch origin pull/<number>/head
```

One retained ref is enough, so a commit that a merge commit or a pull request head keeps needs no tag. Keep the head branch until [`freeze_commits.py`](research/portable-evidence-2026-10-10/freeze_commits.py) lists another ref for the commit. A pre-rewrite commit that must not return to the forge is kept in a [recovery bundle](research/repository-storage-2026-10-08/README.md) instead. The [saved record](research/portable-evidence-2026-10-10/freeze-commits.v1.json) names the ref for each run and the two commits that neither a forge ref nor a bundle keeps.

## Dependency archives

Treat original dependency archives as `kind: "reproducibility"`, one shared content identity per original archive. Restore them under, for example, `artifacts/reproducibility/archives/`, retaining the filenames and hashes that the preparation receipts pin. Pass `artifacts/reproducibility` as the existing `--cache-root` where applicable. Confirm the target's required layout before deleting the old cache; restored dependencies do not supply missing source mirrors.

A rebuilt archive is not automatically equivalent to the original. Use the [versioned cache-replacement protocol](grading-readiness.md) when an original cannot be preserved. Do not change frozen receipts or promise that cache deletion costs only download time.

The [completion inventory](research/evidence-completion-2026-10-08/README.md) records the 15 existing versioned replacements, their frozen target hashes and build-receipt manifests. Its publication status and restoration receipts distinguish prepared packages from shared assets.

## Retire a closed workspace

`prune_workspace.py` keeps its existing valid-only rule and removes only verified unchanged clones/caches. Prefer it before inventory/capture. The separate `retire_workspace.py` can retire an entire closed workspace, including a failed attempt, under a stricter capture contract. It never treats archive presence as investigation closure.

Schedule an exclusive operator maintenance window: stop dispatchers, graders, editors and other writers that can access the candidate. The tool checks same-user processes through Linux `/proc`, serializes retirement operations, and repeats snapshot/activity checks after remote verification and immediately before removal. These checks do not lock out a new unrelated writer. The maintenance window is a required operational precondition, not a claim that process scans eliminate races. Retirement currently refuses hosts without `/proc`; packaging and retrieval also work on macOS.

For each candidate:

1. Export and review all required evidence. Preserve failed diagnostics, disposition, usage and replacement lineage, with a saved investigation closure. Re-run its full inventory with `--hash-all`, saving the single root record from the output's `roots` array under `bench/evidence/inventories/`.
2. Capture **every remaining regular file** under a unique prefix such as `artifacts/workspaces/<id>/`, preserving bytes/modes. The only exclusions are exact known credential locations (`home/.codex/auth.json`, `home/.claude/.credentials.json`) and, for linked worktrees, tracked-identical files plus their `.git` pointer. Symlinks/special files block retirement. Rebuildable files still present must be captured too; this conservative operation does not add a blanket cache-deletion rule for failed attempts.
3. For unknown files, record a hash-pinned classification as evidence/reproducibility with a reason. Unclassified files block retirement. Modified development trees remain protected even if their bytes were captured.
4. Publish and cold-verify the manifest. Save a closure plan conforming to `bench/schema/evidence-retirement.schema.json`. Its evidence references must name small Git-published records establishing inputs, execution, accounting, diagnostics, disposition and lineage; grading also requires the grading category. Each category needs at least one `{ "path": "bench/...", "sha256": "..." }` reference. Large payloads are reached through those records/manifests.
5. Commit and push the plan, full inventory, manifest, restoration receipt and closure evidence. Run from a different checkout against that exact shared commit and branch.

Plan fields:

| Field | Meaning |
| --- | --- |
| `schema_version`, `kind` | `1`; `review`, `grading` or `worktree` |
| `workspace` | Canonical absolute candidate path on the originating host, without aliases or `..` |
| `manifest`, `inventory`, `restoration_receipt` | Repository-relative published records |
| `capture_prefix` | Logical member prefix for the workspace's captured files |
| `investigation`, `closure_reason`, `closed_by` | `closed`, the saved reason and responsible maintainer |
| `maintenance_window` | `true`, only while exclusive maintenance actually holds |
| `accounting_roots` | Nonempty list of every controlling queue root on this host |
| `evidence` | Required category-to-reference arrays described above |
| `classifications` | Optional map from unknown relative path to `{class, reason, sha256}` |

```sh
python3 bench/tools/retire_workspace.py --plan bench/evidence/retirements/example-v1.json \
  --shared-commit <pushed-sha> --shared-branch <branch> --receipt /tmp/example-dry-run.json
python3 bench/tools/retire_workspace.py --plan bench/evidence/retirements/example-v1.json \
  --shared-commit <pushed-sha> --shared-branch <branch> --receipt /tmp/example-removal.json --apply
```

Retirement independently checks remote Git identities/reachability and downloads/restores the release payloads again. It refuses missing diagnostics/categories, changed or uncaptured files, open investigations, active processes, unpublished records, unpushed worktree commits and uncommitted work. Ignored worktree files must be captured. It supports linked worktrees, never the current checkout or the primary clone holding shared Git metadata.

Any controlling reservation that references the workspace blocks removal. This includes priced attempts: `regrade.ledger` still rereads their local dispatch receipts, and removing one would lose the settled charge and reopen its reservation. Resolved zero-charge proofs may also depend on the workspace's existence. The check includes reservation and resolution files, dispatch receipts, workspace/home state and absolute evidence references in zero-charge proofs, including symlink targets. A relative proof reference is refused because its meaning depends on the original controller checkout, which a different checkout cannot infer safely. These workspaces remain until a separate portable accounting change removes and verifies those dependencies. Archiving a receipt or proof JSON alone is insufficient.

Dry runs write a receipt without deleting files. Apply writes a removal-started receipt first and a separate `.completed.json` receipt after success, including verified packages, inventoried allocated bytes, free space before/after and their measured delta. Other activity and hard links can affect that delta; it is not a guaranteed physical-host reclaim measure. A failed removal retains its started receipt and remaining files for inspection. Refusals print their blocking reason; save that output with the migration records and leave the candidate in place. Use a new receipt path for each invocation, then commit the resulting receipts.

## Retention and recovery ownership

Repository maintainers own the release assets and recovery copies. Do not expire or delete assets while a published observation references them. Periodically run `verify` and retain a second copy of the exact packages outside the execution machine. GitHub IDs and hashes detect loss/replacement; they cannot recover deleted bytes.

If an asset is lost, recover its exact bytes from the retained copy, publish under a new versioned release/manifest and record the incident and replacement mapping. Keep the old manifest and failure history. If no copy exists, report evidence unavailable; do not substitute a rerun. No automatic retention job, release deletion, production migration or host retirement is performed by this tooling change. Those are reviewable rollout operations, with a successful review, failed/replacement pair and grading batch verified from a real fresh checkout before bulk migration.

The original local capture, pilot and Git fallback packages have exact copies in the separate `evidence-recovery-2026-10-08-v1` release. The small [recovery index](../bench/evidence/recovery/2026-10-08-v1.json) pins their new asset identities and reuses the original manifests, including every member hash and permission. To recover one source package set, use:

```sh
python3 bench/tools/recover_evidence.py --source local-capture-2026-10-08-v1
```

Use `--path <logical-member>` for selective retrieval and `--offline` for a verified local cache. The other source names are `local-pilot-2026-10-08-v1` and `local-git-fallbacks-2026-10-08-v1`. This recovery release shares the repository and GitHub account with the originals. It covers individual asset or release loss; an independently administered copy is still needed to cover repository, account or provider loss. The [recovery exercise](research/evidence-completion-2026-10-08/recovery-verification.json) downloaded and checked every member while deliberately refusing the original releases.

The [2026-10-08 local publication](research/evidence-migration-2026-10-08/README.md) records the first production release, original-path restoration, cold-consumer checks and remaining migration blockers.
