# Portable evidence and publication scope, 2026-10-10

Part of [issue 94](https://github.com/kamui/code-review-bench/issues/94). This record checks that a checkout at another path can restore the saved evidence of a run, and that each run's freeze commit is still retrievable. It publishes nothing and retires no storage.

## Archive references

Every filed attempt and probe names its transcript archive in `transcript_archive.path`. [`references.py`](references.py) reads all 1,199 of them and writes [`archive-references.v1.json`](archive-references.v1.json).

Before this change, 205 records named the archive by a home-relative path that only the originating checkout resolves, and had no mapping. The collector now maps 204 of them in their run's `transcripts.json`. No attempt record changed.

| Records | Runs | Mapping |
| --- | --- | --- |
| 176 attempts, 8 probes | the seven `2026-10-08-last-push-*` runs | new `transcripts.json` in each run |
| 15 attempts | `2026-09-29-codex-astra-high-writable`, `2026-10-02-claude-ce-opus-5-5-high-selected`, `2026-10-02-codex-ce-sol61-high-selected`, `2026-10-03-codex-ce-sol61-high-selected` | new `transcripts.json` in each run |
| 1 probe | `2026-09-29-codex-builtin` | one entry added to its `transcripts.json` |
| 2 probes | `2026-09-30-selected-prs-review-only` | two entries added, at the path each record names inside its checkout |
| 2 probes | `2026-09-29-codex-sol-high` | two entries added, at the path release storage holds |

The new file of `2026-09-29-codex-astra-high-writable` also lists one probe whose record already held a repository path.

The two `2026-09-29-codex-sol-high` probe records name the transcript cache. Release storage holds their bytes under `artifacts/transcripts/2026-09-29-codex-sol-high-probes/`, which no record names, so the collector maps each record to the one stored archive with its frozen hash.

The owner chose on 2026-10-10 to map these four probes. That choice edits the `transcripts.json` of `2026-09-30-selected-prs-review-only`, which two saved records pin at its earlier hash `71df8281`: [`protected-inputs.v1.json`](../selected-pr-triage-arena-2026-09-30/protected-inputs.v1.json) and [`recovered-files.v1.json`](../grading-evidence-selected-2026-10-02/recovered-files.v1.json). Both records keep that hash, and the 45 earlier entries keep their bytes. The verifiers of both records already failed on `main` before this change, the first on `bench/claims/registry.json` and the second on `claims.reconciliation`, a function that no longer exists. The second now stops earlier, on this file's hash, and it also expects 45 entries where the file now has 47. That cohort's README still warns against running the collector there. The warning describes the collector before this change, and a saved record pins the README too, so it stays.

After the change the inventory counts 1,173 mapped records whose archive is in release storage under the frozen hash, 9 mapped records with the hash mismatch the import already recorded, 16 records that hold a repository path and need no mapping, and 1 unresolved record.

The unresolved record is probe `att-002` of `2026-09-29-codex-sol61-high-clean`. No stored archive has its hash, and the path the record names now holds the archive of attempt `att-002`. The collector now reads probes, so it stops on this record with `Transcript checksum mismatch`. Before this change it skipped every probe. The run's mapping file keeps its bytes, and collecting the run again needs a decision on this probe first.

`file_attempt.py` now records an archive filed inside the checkout by its repository path, so a new record needs no mapping to resolve in another checkout.

The published selection takes attempts of `2026-09-30-selected-prs-review-only`, whose 45 entries are unchanged, and none of the other thirteen collected runs. `bun run data` writes the same 747 data files and 2,286 evidence files, byte for byte, with and without this change.

## Cold restoration

[`verify-cold.py`](verify-cold.py) ran in a new single-branch clone of `main` from GitHub at `/tmp/issue94/cold3/fresh`, with this branch's commits fetched from the local checkout. Bubblewrap hid `/home/jack/.t3` and `/home/jack/development`, which hold the dispatch checkout, the worktrees, the run workspaces and the transcript cache. The clone started with no evidence cache and no archive. The [receipt](../../../bench/evidence/receipts/portable-mappings-2026-10-10-consumers.json) records the run at `85197b9a`.

- None of the 281 origin paths the records name existed.
- The collector rebuilt all fourteen mapping files from release storage, 282 entries, and Git showed no change to any of them. Each archive matched the hash in its attempt record.
- The explorer's evidence export restored and copied five archives: a valid review, a failed predecessor, its replacement, the interrupted attempt with unknown usage, and a probe.
- The deviation records of the last-push runs pin 13 diagnostic files that Git tracks. All 13 matched.
- A changed archive stopped the collector with `Transcript checksum mismatch` and the store with `changed evidence`. With the cached package moved away, offline retrieval stopped with `archive unavailable offline`.
- The clone lacked the freeze commit `149bdbd8`. After `git fetch` of `refs/pull/88/head`, packet selection read all seven frozen manifests and selected the packet of all 60 cohort rows.

`tools/test_evidence_consumers.py` repeats the restoration with synthetic records through the collector, the cleanup resolver and the explorer export. It also checks that a replaced release asset and a missing one stop each consumer with a named reason.

## Freeze commits

[`freeze_commits.py`](freeze_commits.py) fetched every branch, tag and pull request head of `kamui/code-review-bench` and `kamui/skills`, and the Git bundle of the [history-recovery manifest](../../../bench/evidence/manifests/history-recovery-2026-10-08-v1.json). [`freeze-commits.v1.json`](freeze-commits.v1.json) records which refs contain each freeze commit. 53 runs name 26 freeze commits, and `2026-09-24-toy` names none.

| Freeze commits | Runs | Kept by |
| --- | --- | --- |
| 1 (`149bdbd8`) | the seven last-push runs | `refs/pull/88/head` of `kamui/code-review-bench` |
| 13 | 31 | pull request heads of `kamui/code-review-bench`, and `main` in the recovery bundle |
| 9 | 11 | pull request heads of `kamui/skills` |
| 3 | 4 | no forge ref and no bundle ref |

No branch and no tag of either repository contains any freeze commit. The squash merge of pull request #88 left `149bdbd8` off `main`, and the head branch `t3code/resume-issue-60-replacement-runs` no longer exists on the forge. The release tag `evidence-last-push-transcripts-2026-10-09-v1` names the merge commit `a526b620`, which does not contain `149bdbd8`. The pull request head is the one ref that keeps the commit. It is enough for packet selection, as the cold run shows, so nothing depends on the local copy of the branch.

The three commits without a ref:

- `d27a8c24`, `2026-09-30-selected-prs-review-only`. The forge API returns no such commit. `~/development/code-review-bench.old` holds it under `refs/recovery/issue-9-selected-cohort-2026-10-02`.
- `d19fee72`, `2026-10-03-codex-ce-sol61-high` and `2026-10-03-codex-ce-sol61-high-selected`. The forge API still returns the commit, but no ref keeps it, so GitHub can collect it. `~/development/code-review-bench.old` holds it on `t3code/ce-sol61-serial`.
- `61b659b3`, `2026-09-26-x382-destroyed-tests`. The run's manifest records that the commit was never pushed. `~/development/skills` holds it as an unreachable object.

None of these runs froze `packet_replacements`, so packet selection does not need their freeze commits today. Keeping them needs a push from those local clones, which is the owner's decision. Of the 53 run manifests, 39 exist in the tree of their freeze commit.

## Publication scope

This change sends nothing to a release, a site or an external service. The last-push transcripts were published on 2026-10-09, and the [re-cut record](../last-push-recut-2026-10-07/README.md#transcript-archives-moved-to-release-storage-on-2026-10-09) holds the four facts the [storage contract](../../evidence-storage.md#package-publish-and-verify) now asks for:

- Destination: release `evidence-last-push-transcripts-2026-10-09-v1` of `kamui/code-review-bench`.
- Visibility: public.
- Categories: execution (184 transcript archives of the runs and their probes), with their integrity records.
- Authorization: the owner approved publishing the queue's review records, transcripts, diagnostics and cleanup receipts to the public repository, and asked for the release on 2026-10-09. The scan of the 368 session files found no credential.

That authorization names the public repository. It does not cover an external service, and it does not cover the transcripts of a later queue.

## Counts for the last-push batch

The [completion record](../last-push-recut-2026-10-07/replacement-completion.v1.json) holds these. The cold run counted the same 6 failed predecessors, 6 replacements and one attempt with unknown usage in the attempt records.

- 170 valid reviews, 6 failed predecessors and 6 replacements.
- Priced usage of $36.090368 on Claude and $17.779087 on Codex, in list-price equivalent.
- One attempt with unknown usage, `2026-10-08-last-push-claude-opus/att-002`, between $0.233688 and its $5 reservation. Its record still says `metering_status: incomplete`, and the reservation stays.

Cleanup and retirement keep their separate rules. `prune_workspace.py` removes the clone of a valid attempt only and refuses a failed one and one with incomplete usage. `retire_workspace.py` refuses a workspace that a controlling reservation references. A published archive changes neither rule.

## Check this record

```sh
python3 docs/research/portable-evidence-2026-10-10/references.py --check
python3 docs/research/portable-evidence-2026-10-10/freeze_commits.py --git-dir /tmp/freeze-refs.git --bundle <bundle> --check
python3 docs/research/portable-evidence-2026-10-10/verify-cold.py /tmp/portable-receipt.json
```

The first is offline. The second downloads about 430 MB of Git objects into the scratch repository, which grows to about 800 MB once it loads the bundle. `<bundle>` is `artifacts/recovery/shared-before-rewrite-2026-10-08.bundle`, fetched from the history-recovery manifest into a directory outside this checkout, about 395 MB. The third runs from the root of a clean new clone, downloads about 380 MB of evidence packages and writes a new receipt.

Code: `tools/collect_run.py`, `bench/tools/file_attempt.py` (`recorded_archive_path`), `tools/test_collect_run.py` and `tools/test_evidence_consumers.py`.
