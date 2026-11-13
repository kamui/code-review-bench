# Account redaction, 2026-11-13

The owner authorised a one-time exception to the rule that frozen runs and raw evidence stay immutable. Session transcripts recorded the account that ran the benchmark: Claude Code writes the signed-in account's email address and organization id into every session, and Codex writes its account ids and plan type. Transcripts, saved client configuration and research notes also named the two machines that ran reviews. This record says what was replaced, where, and how to check it.

It extends the earlier redaction of grading archives ([`redact.py`](../cohort-rebuild-2026-10-04/redact.py), with records [v1](../cohort-rebuild-2026-10-04/redaction.v1.json), [v2](../cohort-rebuild-2026-10-04/redaction.v2.json) and [v3](../cohort-rebuild-2026-10-04/redaction.v3.json)) to every transcript, to the git history and to the release packages. [The owner's words](authorization.md) are saved beside it.

## What was replaced

| Identifier | Replacement |
| --- | --- |
| The account's email address | `[redacted-email]` |
| The Anthropic organization id | `[redacted-organization]` |
| The account id in a synced-plugin path, and the Codex `creator_user_id` and `creator_account_id` | `[redacted-account]` |
| The Codex `plan_type` value | `[redacted-plan]` |
| The names of the two machines that ran reviews | `host-01` and `host-02` |

Each machine name has a stand-in of the same length, so no file that held only a machine name changed size. The names occurred as hostnames in shell prompts and system reports, in a folder name inside saved Codex configuration, and in prose. A run deviation whose id held a machine name is now `host-host-02-v1`.

Inside SQLite databases and their write-ahead logs every replacement is padded or cut to the original length, because those formats record sizes. Nothing else changed. No tool reads these fields, so no score, grade, usage figure or cost changed.

The owner's home directory path was not replaced, by the owner's decision. It occurs in about 18,500 tracked files, among them 123 attempt records and 146 saved reviews that `bench/grading/current/inventory.json` pins. Replacing it would make 51 of the 194 graded batches stale.

## Git history

The history was rewritten with `git filter-repo`, and the repository was re-created from the result. The earlier repository is kept as a private archive.

- 3,825 objects changed across the history: 947 text files and 2,878 archives, which hold 4,566 changed session files.
- 14 commits that named the account's email address as author or committer now name the owner's public address. None was on `main`.
- Six commit messages named a machine and were changed the same way. Four commit messages lost a `Co-Authored-By` trailer that the repository's rules do not allow.
- Every commit id changed. [`commit-map.v1.txt`](commit-map.v1.txt) lists each earlier id with its new one, for 1,286 commits.
- Four of the 17 branches were carried over: `main`, `feat/add-terra-6-1-high-codex`, `feat/ruling-decision-card` and `t3code/audit-r4-pass`. The owner chose to leave the other 13 in the archive. They are older branches whose work never reached `main`, and 215 of the mapped commits exist only on them, so those 215 new ids name nothing in the new repository.
- Pull request refs do not exist in the new repository. Frozen run commits that `main` does not reach are kept by tags: `freeze-last-push-2026-10-08`, `freeze/2026-10-10-last-push`, `freeze/2026-10-12-last-push-claude-review-code-sonnet`, `freeze/2026-11-13-codex-builtin-terra61-high-matched`, `freeze/2026-11-13-codex-ce-terra61-high` and `freeze/2026-11-13-thermo-nuclear-review-codex-terra`.
- 37 run manifests pin 14 frozen commits the tools read. Their `freeze_commit` values name the new ids ([`freeze-commits.v1.json`](freeze-commits.v1.json)). The other 25 pinned ids belong to `kamui/skills` or to history that the 2026-10-08 rewrite removed, and are unchanged.

At the tip, the records that pin a redacted tracked file were recomputed from the file:

- 987 `bench/regrading/**/evidence.json` receipts, for the archive and each changed member, with the note the earlier redaction wrote;
- 149 `checksums.json` files and one `SHA256SUMS` file;
- 915 records that name, by its hash, one of the 753 text files that held a machine name. A record changed this way can be pinned in turn, so the update was repeated until no pin was stale.

[`tracked-files.v1.json`](tracked-files.v1.json) gives the earlier and later hash of each archive and checksum row. [`machine-name-pins.v1.json`](machine-name-pins.v1.json) lists the machine-name files and the records repinned for them.

`bench/grading/current/inventory.json` and `docs/python-migration/grading-replay.v1.json` were regenerated, because they record commit ids and the hashes of run manifests and receipts.

## Release packages

A new repository issues new release and asset ids, so every evidence manifest was published again.

- 31 packages in 24 manifests hold a changed member. Each was repacked with `bench-tools evidence pack` from its redacted members, keeping every path and mode. [`packages.v1.json`](packages.v1.json) maps each earlier package to its replacement, and [`release-members.v1.json`](release-members.v1.json) lists each changed member and the changed files inside it.
- 25 packages hold no identifier and were uploaded again byte for byte. They are the dependency caches and toolchains.
- Six manifests are not republished and now name the private archive as their repository: the three `history-recovery-*` manifests, `local-capture-2026-10-08-v1`, `local-git-fallbacks-2026-10-08-v1` and `withheld-2026-10-08-v1`. Git bundles and compressed git objects cannot be redacted, and the other two stores name each object by its own hash. The owner agreed to keep them private.

Records that follow a changed archive now name the redacted bytes:

- 1,720 entries in `bench/runs/*/transcripts.json`;
- all 288 transcript entries in `bench/import-manifest.json`, in both `sha256` and, where it matched before, `expected_sha256`;
- 1,186 `bench/regrading/**/evidence.json` receipts for archives held in releases.

Records that quote an archive's hash as history are unchanged and name the earlier bytes: publication plans, network scans, prune receipts, `workspace-pruned.json` files, dispatch checkpoints and the fixture manifests under `bench/fixtures/`. `release-members.v1.json` gives the later hash for each.

Every `attempt.json` keeps its `transcript_archive.sha256`, the hash of the archive as it was first saved. For a redacted archive it differs from `transcripts.json` and from the published bytes. Do not run `evidence collect` or `prune-workspace` on those runs: both compare an archive with the frozen hash.

## Issues and pull requests

GitHub numbers issues and pull requests in one sequence, and commits, rulings and records cite those numbers. An issue was created at every number from 1 to 494, in order, before anything else:

- 142 issues were created again with their title, body, labels, comments, sub-issue links and state;
- 298 pull request numbers are closed issues with the pull request's title and description, the date it merged and, where the merge commit survives, its new id;
- 54 numbers are unused. The owner chose not to carry over the 15 issues closed as not planned, the 34 Dependabot pull requests and 5 other pull requests closed without merging. Each was created to hold its place and then deleted.

The same replacements were applied to this text, and commit ids in it were mapped to the new ones. Every date on GitHub is now the day of the re-creation; the original dates are in the text. The pull requests themselves, with their diffs and reviews, are in the archive.

## What did not change

A data export built from the earlier history and one built from the redacted repository are equal in every field except `evidence.datasetHash`. That hash covers the inventory, which records the run manifests' new hashes.

## Verification

Run before publication on a clone holding only redacted evidence, with no network:

| Check | Result |
| --- | --- |
| A second redaction pass over every git object | 0 objects would change |
| The same pass over the working tree, restored evidence and site output | 101,220 files, 0 would change |
| `evidence fetch --offline` for the 16 manifests CI reads | verified |
| `fetch-freeze-commits` | 13 freeze commits, 0 absent |
| `current-grading check` | valid, 615/615 admitted reviews assessed |
| `calibration check` | valid, 87/87 families and 22/22 pre-existing problems labelled |
| `import-benchmark --check` | 2,775 source files and 288 archives verified, 9 superseded mismatches as before |
| `claims check`, `evidence pinned` | pass |
| `grading_replay`: inventory, validator replay and full mapping replay | pass, 946 restorable archives as before, 333 of 333 mapped |
| `export-explorer`, `bun test ./src`, type check and site build | 16 tasks, 16 configurations, 653 attempts; pass |
| `cargo fmt`, `cargo clippy`, the `behavior_contracts` and `runtime` tests | pass |

After publication the same checks were run from a fresh clone of the new repository, restoring evidence from the new releases. All passed, and the redaction pass over that clone covered 101,228 files with 0 to change.

Tests that need persistent disk, a real `node` binary or a signed-in client did not run on the build machine. CI in the new repository runs them.

## Limits

- The check finds the literal values, the Codex fields and the two machine names in lower case. It does not find an identifier written in a form nobody anticipated.
- 30 git bundles and packs in the dependency caches hold third-party repositories and were not opened. The other files in those caches were searched, and none named a machine.
- A write-ahead log checks each frame with a checksum, so SQLite will ignore the redacted frames of the logs that changed. Nothing reads those databases.
- The home path remains, as described above.
