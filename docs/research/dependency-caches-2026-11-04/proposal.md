# Proposal: no host keeps benchmark data long-term

## The short answer

Of the 100 GB+ local data, under 10 GB has to be kept anywhere. The rest is build output, extracted copies, download caches and duplicates, and it can always be rebuilt. The part that has to be kept goes once to the repo's GitHub Releases, named by its SHA-256. Each job downloads only its own task's files when it needs them, checks them, and the host can delete them afterwards. The repo already has the tools for this (`bench-tools evidence pack/publish/verify/fetch`) and has used them for 45 files (4.5 GB). What's missing is one rule, "nothing may be pinned until it's published", plus a small fetch step.

## What exists (measured on siamese, 2026-11-04)

| Data | Size | Keep? | Published? |
|---|---|---|---|
| Records (runs, grades, claims, rulings, manifests) | git pack 377 MiB | keep | git |
| Transcripts, run payloads, older grading archives | ~0.6 GB local | keep | yes |
| Dependency archives, replacements (15) | ~2.0 GB | keep: rebuilds are not byte-identical | yes |
| **trpc archive `afd66914` (rebuilt today)** | 386 MB | **keep: the only copy** | **no** |
| trpc archive `8f53201c` (Oct 22) | 368 MB | was onigiri-only | **no; probably gone** |
| Toolchain supplements | 0.7 GB | keep | yes |
| Git mirrors (16 here) | 2.4 GB | rebuildable from upstream, slowly and only while upstream keeps the commits | no |
| Queue state (status, reservations, keys) | ~8 MB | keep: it is the spend ledger | no |
| Grading workspaces | ~1.2 GB | keep while their queue is open, then retire | archived per attempt |
| Claude client copies (4 identical) | 1.27 GB | rebuildable; **never publish (vendor binary)** | no |
| `.cache/evidence`, `rust/bench-tools/target`, `node_modules`, staging clones | ~7 GB | scratch | n/a |

Uncommitted records exist only here: the trpc rebuild record, the second-pass setup, audit round 2, and rulings v8 and v9. They go to git with the next push.

## Design

- **Git:** small records, as today. That includes manifests, build receipts, mirror receipts, and a snapshot of each queue's state files.
- **GitHub Releases:** every large pinned input, published once and named by hash, in the layout of a cache folder:
  - `artifacts/reproducibility/archives/<task>.<sha256>.tar.gz`. bench-tools already prefers these names, and the fetch code refuses two different files at one path, so hash names are required.
  - `artifacts/reproducibility/mirrors/<task>.bundle`: one `git bundle` per mirror. A test round trip restored the same refs at about the same size. Kubernetes is about 1.25 GB, under the 2 GiB per-file limit with `--part-bytes 2000000000`.
  - The repo is public, so releases are public. Never publish credentials, account state or client binaries.
- **One manifest per task**, so a job downloads only its task (about 0.5 GB at most).
- **Never stored:** build output, download caches, extracted clones, client binaries.
- **Total published:** 4.5 GB today, plus about 0.4 GB for `afd66914` and about 2.4 GB of mirror bundles, so **about 7.3 GB**. Each new task adds at most about 0.5 GB, a review run 5–25 MB, and a grading cohort 20–110 MB.

## How this prevents today's failure

1. **Publish before pin (enforced).** A new read-only check, `bench-tools evidence pinned`, fails when a `target.json` or cache-replacement manifest pins a hash that no published manifest contains. It runs in CI next to `bun run verify:import`. It would have flagged `8f53201c` on Oct 22.
2. **Fetch by hash.** If `8f53201c` had been published, siamese would have downloaded the original archive instead of rebuilding a byte-different one.

## Plan

**Now (owner OK needed, no code change):**
1. Publish `afd66914` (the only copy, which the running audit depends on) and the 16 mirror bundles to a new release, e.g. `evidence-dependency-caches-2026-11-04-v1`. The commands are `evidence pack`, `check-package`, `gh release create`, `evidence publish` and `evidence verify`, with `kind: "reproducibility"`, one selection per task, and the four facts written into the record first.
2. Push the uncommitted records and a queue-state snapshot of `regrade-v5-2` and `-3`.
3. On onigiri, check what survived:
   - `sha256sum ~/.t3/bench-cache/archives/j-trpc-5017* ~/.t3/bench-cache/staged/*`, for `8f53201c`;
   - the first host's `.local/regrade-v5/queues/rebuilt`, which holds the 9 unsettled attempts and the $36.07 spend record.

   Publish or snapshot anything found before deleting more.
4. Cold check: in a fresh folder, `evidence fetch` a small task and kubernetes, restore the mirror from its bundle, and run `provision check`/`smoke`.

**Code (one PR, after the audit queue closes):**
- **Step 1, safe at any time:**
  - The `evidence pinned` check (`main.rs`, using the manifest reader in `evidence.rs`).
  - A standalone `provision fetch --target T [--cache-replacements M] --cache-root R` (`provision.rs`, `recorded_archives` + `evidence::resolve`), which downloads, verifies and places a task's archive and mirror.
- **Step 2, between queues only:** make `restore_cache`, `space_needed`, `Supplement::size`, `make_clone`/`check` and `grade.rs::preflight_target` call the fetch automatically when a file is missing. Any change to bench-tools changes the pinned runtime and forces a new queue authorization, so don't ship it mid-queue.
- **Docs and ignore rules:**
  - `.gitignore` adds `/artifacts/reproducibility/mirrors/`.
  - `docs/evidence-storage.md` and `docs/grading-readiness.md` gain the publish-before-pin rule.

**Later (owner OK for each deletion):**
- Delete download caches and build output at any time.
- Delete a host's cache folder once everything in it verifies as published.
- Retire closed queues with `retire-workspace`.
- Portable accounting, so grading workspaces can be retired, is a separate change.

## Alternatives considered

- **Git LFS:** rejected. It has a small quota with metered billing, adds a dependency to every clone, and the storage contract rules it out.
- **R2/S3/B2:** deferred. It could be private, but it needs credentials on every host and a new bill. It's the right place for a second, independent copy.
- **NAS share or rsync:** rejected as the shared store, since onigiri can't reach the NAS. It's fine as a backup.
- **One cache host:** rejected. That's today's failure with a name.
- **Rebuild instead of store:** rejected for archives, because the trpc rebuild came out byte-different. It's acceptable as the fallback for mirrors.

## Synthesis note

- **Base:** candidate 1 (Opus 5.5 high). Its inventory survived the spot-checks, and it had the complete list of places to change. The judge (Fable 5.1) agreed, 28 to 27.
- **Grafts from candidate 2 (Sonnet 5.5 high):**
  - the fetch command ships first, and the automatic fallback only between queues, because of the runtime pin;
  - one `git bundle` per mirror instead of thousands of loose files;
  - the onigiri queue-state check and the queue snapshots;
  - the one-hash-per-path rule as the reason for hash names.
- **Fixed:** the build folder is 3.4 GB, not 4.3; `afd66914` is 386 MB, not 368.
- **Verified:**
  - `archive_path` prefers hash-keyed names (`provision.rs:415`);
  - `evidence::resolve` refuses two hashes for one path (`evidence.rs:1257`);
  - the bundle round trip of the q-soba mirror gave identical refs.
- **Not verified:** the kubernetes bundle size, and a full cold `provision check` from release files.
