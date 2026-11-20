# Host configuration redaction, 2026-11-14

An audit of the published evidence found no credentials, no other people's files and no host listings in any reviewer transcript. It found two
kinds of the owner's host configuration in published files: the owner's personal agent-instructions file, and the list of skills installed on the
machines that ran reviews, with the Windows account name in their paths. The owner chose to redact both, in this order: prepare everything locally,
stop before publishing, and redact the instruction block in the Codex adjudication ("ruling") sessions rather than withdraw their packages.
On 2026-11-20, after a running benchmark had finished, the owner approved the publication below.

## What was replaced

| Where | Replacement |
| --- | --- |
| The owner's personal instructions in Claude Code `instructions` attachments, Claude system reminders and Codex project-doc messages | `[redacted-instructions]` |
| The installed-skill list in a Codex session's `### Available skills` section | `[redacted-skill-list]` |
| The Windows account in `/mnt/<drive>/Users/<account>`, and the skill folder name under that account's `iCloudDrive/<machine>/.agents/skills/` | `[redacted-user]`, `[redacted-skill]` |
| The skill folder name under the service user's `.t3/agents/.agents_skills/` and `.t3/agents/.codex/skills/` | `[redacted-skill]` |

`bench-tools redaction` carries these as structure rules in `rust/bench-tools/src/redaction.rs`, with tests, and
[the evidence contract](../../evidence-storage.md#redaction) describes them. They need no private value.

## What changed in this tree

- 818 tracked files: 761 `codex-config.toml`, 32 `skill-pin.json`, 3 launcher traces and 22 research files.
- Records that name a changed file by hash, repinned: 629 `bench/regrading/**/evidence.json` receipts (archive hash, inner member hashes and a
  `redaction` note), 469 `dispatch.txt` and 469 `skill-attempt.json` (the `codex-config.toml` hash), 28 `bench/runs/*/transcripts.json` and
  `bench/import-manifest.json`. A second pass found nothing that names those records.
- Records that quote a hash as history are unchanged: this directory's sibling `account-redaction-2026-11-13`, `grading-evidence-selected-*`,
  `finding-threshold-*`, `owner-rulings-*` and the retirement inventory name the earlier bytes.
- `bench/grading/current` and `bench/regrading/audit-v2-r4-2026-11-11` are untouched. No grading archive they pin changed.

## Packages

[`packages.v1.json`](packages.v1.json) maps each earlier package to its replacement for 14 manifests, and
[`release-members.v1.json`](release-members.v1.json) lists the 1,695 members whose bytes changed, with the earlier and later hash.
The replacements were built with `bench-tools evidence pack` from members redacted in a copy of the fetched evidence; every package was verified
by the pack step and every changed `.jsonl` member still parses line for line as before.

`repository-payloads-2026-10-08-v1` held its members in nine packages; six of them held a changed member. The replacement holds all 3,508
members in one package. The other 13 manifests had one package each and still do.

## Publication

- Destination: the existing release tag of each of the 14 manifests in `kamui/code-review-bench`, beside the earlier assets. The new assets are
  named by their own hash, so no asset was replaced.
- Visibility: public.
- Categories: Execution (reviewer transcripts), Grading (grader and ruling sessions) and Integrity (the manifests and restoration receipts).
- Authorization: the owner, 2026-11-20: "upload the 14 replacement packages, verify each by cold fetch, publish their manifests and open the data
  pull request". The earlier assets are deleted only after every replacement verifies, and only after the owner is told.

`bench-tools evidence publish` uploaded each package, read it back and restored every member before it wrote the manifest;
`bench-tools evidence verify` then fetched each package cold into an empty directory and wrote the receipt in `bench/evidence/receipts/`.

## Not covered

- Git history before this commit still holds the earlier bytes of the 761 configs and the other tracked files. The 2026-11-13 pass rewrote the
  history and re-created the repository; this one did not.
- The rules find only these forms. A skill name or instruction text written in another form passes.
- `bench/runs/2026-09-29-codex-sol61-high-clean/transcripts.json` holds one `missing` entry whose frozen hash matches neither package. It was like
  that before this change.
- `docs/research/candidate-rulings-2026-11-07/o-astro-16079/vercel-run/RESULTS.md` keeps its bytes. Its hash is pinned as grading evidence and its
  redaction waits for the owner's decision, so `bench-tools redaction check --require-values` flags that file and no other.

## Verification

- `cargo test --lib redaction`: 23 passed, including four new tests for these rules.
- `bench-tools redaction check --require-values` on the tracked tree and on the redacted copy of every evidence package: 0 findings.
- A phrase scan for the instruction text, the skill names and the Windows account over the redacted copy: no matches.
