# Rule-version-4 regrade: grading archives release draft

This folder prepares a release of the regrade's grading archives. Nothing here is published. No release exists, nothing was uploaded, and no tracked file outside this folder changed. Publishing needs the owner's approval for this release (see [the question](#the-question-for-the-owner)).

Prepared on 2026-10-30. Each fact is labelled **(run)** when a command here produced it, **(read)** when it comes from a file or record, and **(estimate)** when it is a judgement.

## Name and tag

| Item | Value |
| --- | --- |
| Release tag | `evidence-rule-v4-regrade-2026-10-25-v1` |
| Release title | `Rule v4 regrade grading archives v1` |
| Manifest | `bench/evidence/manifests/rule-v4-regrade-2026-10-25-v1.json` |
| Restoration receipt | `bench/evidence/receipts/rule-v4-regrade-2026-10-25-v1-restoration.json` |
| Visibility | public |
| Categories | Grading, Integrity, and Accounting and lineage |

The names follow the rule-version-3 regrade's releases, such as `evidence-rule-v3-regrade-2026-10-23-v1` and its manifest `rule-v3-regrade-2026-10-23-v1.json`. **(read)** The tag is free: `gh release view evidence-rule-v4-regrade-2026-10-25-v1` exited 1 with "release not found", and the repository's release list has no `rule-v4` tag. **(run)**

One release and one manifest cover all 15 queues. The rule-version-3 split came from separate approvals over two days, not from a size limit. Its first release already put three queues in one manifest. **(read)** The selection is 108 MB, well under the 1 GiB package limit, so `pack` made one package. **(run)**

Categories: the first rule-version-3 release used Grading and Integrity. The audit-v2 release added Accounting and lineage, and Execution, for its failed attempts. **(read)** Four of these archives are failed attempts whose reservation and dispatch log record an unknown charge, so this draft adds Accounting and lineage. **(estimate)**

## Files and sizes

The selection is every `evidence.json` under `bench/regrading/rule-v4-regrade-2026-10-25*/` whose archive no manifest in `bench/evidence/manifests/` lists. There are 354. None is published by path or by hash. Every archive is present, sits beside its `evidence.json`, and matches the SHA-256 that file pins. **(run)**

- 354 archives, 108,364,976 bytes in all (103.3 MiB). **(run)**
- 350 are saved assessments. 4 are failed attempts: attempt-1 of four `n-ripgrep-2957` batches in queue `plain-3`, which the provider's usage limit stopped (`diagnosis.plain-3.v1.json`). **(run, read)**
- The 350 saved archives match the 350 receipts that `docs/python-migration/grading-replay.v1.json` lists as "no evidence manifest lists the archive". **(run)**
- Largest archive: `bench/regrading/rule-v4-regrade-2026-10-25-selected-7/2026-10-02-claude-thermo-opus-5-5-high-selected/v-django-17914/attempt-1/evidence.tar.gz`, 620,517 bytes. **(run)**
- The packed asset is 107,581,029 bytes (`f1015e0c65dbfa6f84b1de9b8954e8285d3ef4b0e8c4b2e1d13680f54cbc2c7d.tar.gz`). **(run)**

| Queue | Archives | Bytes |
| --- | ---: | ---: |
| `rule-v4-regrade-2026-10-25-plain` | 28 | 8,262,042 |
| `rule-v4-regrade-2026-10-25-plain-2` | 28 | 8,500,775 |
| `rule-v4-regrade-2026-10-25-plain-3` | 18 | 5,141,826 |
| `rule-v4-regrade-2026-10-25-rebuilt` | 123 | 36,872,337 |
| `rule-v4-regrade-2026-10-25-rebuilt-2` | 2 | 584,573 |
| `rule-v4-regrade-2026-10-25-rebuilt-3` | 1 | 269,290 |
| `rule-v4-regrade-2026-10-25-rebuilt-4` | 58 | 18,379,844 |
| `rule-v4-regrade-2026-10-25-rebuilt-5` | 14 | 4,375,213 |
| `rule-v4-regrade-2026-10-25-selected` | 20 | 6,539,567 |
| `rule-v4-regrade-2026-10-25-selected-2` | 8 | 2,549,337 |
| `rule-v4-regrade-2026-10-25-selected-3` | 15 | 3,410,446 |
| `rule-v4-regrade-2026-10-25-selected-4` | 2 | 521,709 |
| `rule-v4-regrade-2026-10-25-selected-5` | 1 | 272,268 |
| `rule-v4-regrade-2026-10-25-selected-6` | 27 | 8,934,089 |
| `rule-v4-regrade-2026-10-25-selected-7` | 9 | 3,751,660 |
| **Total** | **354** | **108,364,976** |

The archives are untracked, not ignored, in this checkout. **(run)**

### Attempts left out

The queue folders under `.local/regrade-v4/*/queue/` hold 371 attempts. 17 of them have no archive and no `evidence.json`. Each ran one Codex session and has no archived result. 14 ended with "Codex session exit 1", 6 of them also with "no verdicts.json written". 3 logged a grade but were never archived. Their receipts and session transcripts are 89 files, 9,768,855 bytes, and they stay on this machine only. **(run)** The audit-v2 release did publish its failed attempts' receipts and transcripts, "so rerun costs are on record". **(read)** This draft leaves the 17 out because they are not archived. Adding them would need a selection change and the owner's say. **(estimate)** The same review below found no email or credential in them. **(run)**

## What the archives contain

Each archive is the grader's workspace as `bench-tools regrade archive-attempt` saved it. Its `evidence.json` lists every member and names two exclusions: "rebuildable clone and cache" and "home account configuration and credentials". **(read)** The 354 archives hold 9,976 members, 432,085,354 bytes unpacked. Every member is a regular file with a safe path; none is a nested gzip. **(run)**

| Member | In archives | Bytes unpacked | What it is |
| --- | ---: | ---: | --- |
| `work/home/.codex/sessions/**/rollout-*.jsonl` | 354 (one each) | 250,623,113 | The grader's Codex session transcript |
| `work/stdout.jsonl` | 354 | 107,491,207 | The Codex exec event stream |
| `work/rubric.md`, `work/prompt.md`, `work/packet.md` | 354 each | 26,697,611 | Grader instructions and the task packet |
| `work/reviews/blind-*.md` | 1,198 | 3,421,590 | The blinded reviews the grader read |
| `work/claims.md`, `work/references.json` | 354 each | 7,534,737 | Claim list and reference problems |
| `work/verdicts.json` | 354 | 13,181,819 | The grader's verdicts |
| `work/validator/inputs.json` | 354 | 3,980,777 | Validator inputs |
| `key.json` | 354 | 3,546,605 | The blinding key |
| `work/home/.codex/config.toml` | 354 | 11,039,637 | The session configuration grade dispatch wrote |
| `reservation.json`, `prepare.log`, `dispatch.log`, `map.log` (350), `work/dispatch.json`, `work/timing.json` | 354 | 1,410,116 | Receipts: reservation, dispatch, charge and mapping |
| `work/command-policy.json`, `work/command-audit.jsonl` (290), `work/policy-audit.jsonl`, `work/execution-policy.md`, `work/grading-hosts.v1`, `work/clean-context.json`, `work/workspace-pruned.json` (350), `work/stderr.txt` | 354 | 3,158,142 | Policy, audit and isolation records |

**(run)** for counts and bytes, **(read)** for what each file is.

Grader workspaces: **yes**, without the clone, its caches and the client's account files. Codex session transcripts: **yes**, one per archive. **(run)**

## Email addresses

Every member of every archive was read in memory and searched, not a sample. **(run)**

- `kamuigt@gmail.com`, the owner's account email: **0 occurrences** in the 354 archives and in the 17 left-out attempts. **(run)**
- `jack@jackchu.com`, the git author email: **0 occurrences** in both. **(run)**
- Control: the same search found the owner's email 81 times in 27 of the 28 published rule-version-3 `plain` archives, whose Claude grader sessions carry it. So the search works. **(run)**
- Other addresses: 48 distinct, all from the reviewed projects or the vendor. 2 are noreply addresses, 18 use example or test domains, and 28 are other test-data or upstream addresses. 25 of the 48 are not in the published rule-version-3 grading archives. 6 of those are real people and are in no tracked file: four `jsdom` maintainers in a `package.json` under `node_modules` (`l-bokeh-9232`), the `ripgrep` author in its man page (`n-ripgrep-2957`), and the `zod` author in its `package.json` (`j-trpc-5017`). Each appears in a session transcript and `work/stdout.jsonl` because the grader read that file. All six are in those projects' public sources. **(run)**

## Credentials

- No credential file: no member is named `auth.json`, `.credentials.json`, `.claude.json`, `credentials`, `credentials.json`, `.netrc`, `.npmrc`, `.pypirc`, `id_rsa`, `id_ed25519`, `.env`, `hosts.yml`, `.git-credentials`, `installation_id` or `models_cache.json`. **(run)**
- `config.toml`: 354 members, one per archive, written by grade dispatch. Its keys are model catalog path, feature switches, sandbox and approval settings, developer instructions, the grading tool server's command and arguments, and the `skills.config` list. No token, key or account state. **(run)**
- Live values: the 58 string values of 20 or more characters in this host's `~/.codex/auth.json` and `~/.claude/.credentials.json` were compared with every member in memory. The only match is `tokens.account_id`, once per session transcript. It is the account ID the session header records as `creator_account_id`. It is an identifier, not a credential. No access, refresh or ID token appears. These files hold today's tokens; tokens that rotated before today cannot be checked this way. **(run)**
- Token patterns: the 13 patterns of `evidence-publication.v4.json` (rule-version-3), plus OAuth JSON keys (`access_token`, `refresh_token`, `id_token`), Google OAuth tokens and Claude OAuth tokens. Loose searches for `Bearer `, `sk-`, `ghp_`, `gho_` and `github_pat_` ran too. **(run)** Every match is a false positive:

| Pattern | Matches | Where | What it is |
| --- | ---: | --- | --- |
| `jwt` | 64 | `r-base-ui-5460` | The Netlify deploy-preview QR-code link in the task's public pull request, as in every earlier release |
| `assigned-secret` | 180 | `i-requests-6667` (96), `y-django-16631` (66), `s-seaweedfs-10735` (18) | Source code the grader read, such as `password = get_auth_from_url(...)` and `password = models.CharField(...)` |
| `url-credential` | 126 | `i-requests-6667` (78), `s-seaweedfs-10735` (48) | Documentation and test examples such as `http://user:pass@10.10.1.10` and `redis://user:password@localhost` |
| `openai-token` | 2 | `j-trpc-5017`, `s-seaweedfs-10735` | The letters `sk-` inside the model's `encrypted_content` reasoning blob, which is opaque ciphertext |
| loose `Bearer ` | 111 | `p-hono-5067` (102), `i-requests-6667` (9) | Test code: `Bearer token123` and `Bearer XXX` |
| loose `ghp_` | 18 | `q-soba-195` | The documentation placeholder `GITHUB_TOKEN="ghp_..."` |
| `gho_`, `github_pat_`, OAuth keys, other patterns | 0 | | |

**(run)** for counts, **(read)** for the reading of each match.

## Account identifiers

These are what an earlier release shows the owner, so they are listed for comparison. **(run)**

- One `creator_user_id` and one `creator_account_id`, in the header of each of the 354 session transcripts. They match, by hash, the values in the 430 published rule-version-3 grading archives. **(run)**
- `plan_type` in the rate-limit records: two values, `prolite` 4,712 times and `plus` 694 times. Earlier releases showed one value. **(run)**
- No organization ID, no `credential_org` or `userEmail` attachment, and no text of the owner's personal instructions or memory index. **(run)**
- Windows profile path with the login `kamui`: 75,756 mentions, all in the 354 `config.toml` files. They are paths of the owner's personal skills, which `skills.config` turns off. The 107 skill names are the same as in the published audit-v2 archives. No skill file is included. **(run)**
- Local home path `/home/jack`: 20,906 mentions in 3,273 members. **(run)**
- Client installation IDs: none of the 371 session installation IDs on disk, and not this host's own ID, occurs in any member. **(run)**

## Checks

| Check | Command | Exit | Result |
| --- | --- | ---: | --- |
| Selection | `python3 -I docs/research/rule-v4-regrade-2026-10-25/release-draft/capture_evidence.py --out <scratch>/capture` | 0 | 354 archives for 354 subjects; every hash matches its `evidence.json` **(run)** |
| Pack, local only | `BENCH_RUNTIME_STORE=/home/jack/.cache/regrade-v4-store bench/tools/bench-tools evidence pack --selection <scratch>/capture/selection.json --out <scratch>/package` | 0 | `pack: verified`; one package, 354 members **(run)** |
| Package check | `bench/tools/bench-tools evidence check-package <scratch>/package` | 0 | `check-package: verified` **(run)** |
| Repeatable pack | the same `pack` into a second folder | 0 | identical asset and manifest hashes **(run)** |
| Restored bytes | the package extracted into an empty folder and compared | 0 | all 354 files byte-identical to the sources and to their `evidence.json` hashes; no extra file **(run)** |
| Credential and identity review | `python3 -I docs/research/rule-v4-regrade-2026-10-25/release-draft/review_archives.py <inventory> <out>` | 0 | as above; a second run gave identical counts **(run)** |
| Left-out attempts | the same script on the 17 attempts' 89 files | 0 | no email, no credential; same false positives (Netlify link, `requests` and SeaweedFS source) **(run)** |
| Network reach | `python3 -I docs/research/rule-v4-regrade-2026-10-25/release-draft/network_scan.py <inventory> <out>` | 0 | see below **(run)** |
| Cutoff leak check | `bench-tools leak-check scan` | not run | see below |
| Patches | `git apply --check pages.yml.patch` and `git apply --check gitignore.patch` | 0 | both apply to the current tree **(run)** |

The `<inventory>` is a list of the 354 archive paths and hashes, made by the same rule as `capture_evidence.py`. Scratch output went to `/home/jack/.cache/release-draft-scratch/`, which is deleted.

**Network reach.** The evidence docs do not require a network scan for grading archives, and the rule-version-3 grading releases did not run one. **(read)** It ran anyway. `network_scan.py` feeds each session transcript to the unchanged 2026-10-22 rules in `docs/research/network-scan-2026-10-22/scan_transcripts.py`. It parsed 57,919 lines and 10,629 tool calls, with 0 bad lines. **(run)** Every tool call is one of the six tools of the `grading` server: `inspect`, `run`, `validate`, `write_verdicts`, `write_scratch` and `edit_verdicts`. The scanner marks each as a strong "mcp_tool" event, because its rule flags any MCP call. There is no web search, no shell call and no other strong event. **(run)** Commands through the `run` tool used `git` only for `status`, `diff`, `show`, `log` and `rev-parse`. The only URLs in their arguments are `example.invalid`, `example.test`, `localhost` and `proxy.invalid`. **(run)** The sessions ran with web search off, a read-only sandbox with network "restricted", and a hosts file of `localhost` only. **(read)**

**Cutoff leak check.** `bench-tools leak-check scan` compares reviewers' findings with text written after each task's cutoff. It reads run attempts under `bench/runs/`, not grading archives, and needs the task mirrors. A grader writes verdicts, not reviews. The reviews it read are the selected attempts that the 2026-10-22 sweep and each later run's checks already covered. The rule-version-3 grading releases did not run it either. **(read)** So it was not run. **(estimate)**

## Draft files

- [`rule-v4-regrade-2026-10-25-v1.unpublished.json`](rule-v4-regrade-2026-10-25-v1.unpublished.json): the manifest `pack` wrote, SHA-256 `93aa93e7826e846bdb94b0300a90d3a5915190d23c2ca8fc8ed26a99a3986169`. It has no `publication` block. `publish` writes the real manifest, with release and asset IDs, to `bench/evidence/manifests/`. It is kept here so the grading replay inventory test does not read it. **(run)**
- [`capture_evidence.py`](capture_evidence.py): writes the selection. It only reads.
- [`review_archives.py`](review_archives.py) and [`network_scan.py`](network_scan.py): the review and the network scan. They only read. The review keeps match text in a separate `.hits.json` file, which should stay local.
- [`pages.yml.patch`](pages.yml.patch): adds the new manifest to the Pages workflow's fetch step and its `archive check-packages` step, next to the rule-version-3 manifests.
- [`gitignore.patch`](gitignore.patch): ignores the 354 archive paths after publication, as the rule-version-3 releases did.

## What publishing would involve

1. Record the owner's approval in `docs/research/rule-v4-regrade-2026-10-25/evidence-authorization.v1.json`: destination (`kamui/code-review-bench`, tag above), visibility (public), categories, and the answer with its date. The evidence storage rules require these four facts before publishing.
2. Rerun `capture_evidence.py`, `review_archives.py` and `pack`. The asset should hash to `f1015e0c…2c7d` and the manifest to `93aa93e7…6169` again.
3. `gh release create evidence-rule-v4-regrade-2026-10-25-v1 --repo kamui/code-review-bench --title 'Rule v4 regrade grading archives v1' --notes 'Immutable benchmark evidence payloads.'`
4. `bench/tools/bench-tools evidence publish --manifest <package>/manifest.json --out bench/evidence/manifests/rule-v4-regrade-2026-10-25-v1.json`
5. `bench/tools/bench-tools evidence verify --manifest bench/evidence/manifests/rule-v4-regrade-2026-10-25-v1.json --receipt bench/evidence/receipts/rule-v4-regrade-2026-10-25-v1-restoration.json`, then `evidence fetch` of the whole manifest into an empty root.
6. Apply `pages.yml.patch` and `gitignore.patch`.
7. Regenerate `docs/python-migration/grading-replay.v1.json` after the fetch: `(cd rust/bench-tools && cargo test --locked --test grading_replay -- --ignored --exact the_replay_inventory_accounts_for_every_receipt)`. The 350 receipts now listed as not restorable should become restorable. The validator and mapping replays (`the_validator_replays_every_restorable_archive_identically`, `the_mapping_replays_every_restorable_archive_identically`) then run on them in CI and must pass. They have not run on these archives yet.
8. Write `evidence-publication.v1.json` beside the approval, in the shape of the rule-version-3 records. Run `bun run verify:import`, then commit and push the manifest, receipt, patches and records.

## The question for the owner

Plain-language text the coordinator can show before one question:

> The regrade under rule version 4 left 354 grading archives on this machine, 108 MB in all. Each holds one grader's workspace: its instructions, the blinded reviews it read, its answers, its receipts and its Codex session transcript. 350 are finished grades and 4 are attempts that hit the usage limit. They are not in git. The storage rules put archives like these on a public GitHub release, as with the rule version 3 regrade. Your email address is not in any of them, and neither is the git author address. No password, token or login file is in them. Like the earlier Codex grading release, they show your account and user IDs, your Windows login name in skill folder paths, the names of your skills, and local folder paths. They also show six upstream maintainers' addresses from public project files, which no earlier release carried. 17 other failed attempts were never archived and are not included.
>
> **Question:** Publish the 354 grading archives to the public release `evidence-rule-v4-regrade-2026-10-25-v1`?
>
> - **Publish (Recommended):** one public release, packed like the rule version 3 releases, without changing the archives' bytes.
> - **Keep local for now:** leave them on this machine and decide later. Nothing goes into git.
