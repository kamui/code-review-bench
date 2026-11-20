# Host configuration redaction, 2026-11-14

An audit of the published evidence found no credentials, no other people's files and no host listings in any reviewer transcript. It found two
kinds of the owner's host configuration in published files: the owner's personal agent-instructions file, and the list of skills installed on the
machines that ran reviews, with the Windows account name in their paths. The owner chose to redact both, in this order: prepare everything locally,
stop before publishing, and redact the instruction block in the Codex adjudication ("ruling") sessions rather than withdraw their packages.

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

**Not done:** the replacement packages are not uploaded, and `bench/evidence/manifests/*.json` still name the earlier packages. Until each package is
uploaded and its manifest published, the repinned records name members that no manifest holds. The manifests also need the owner's approval
before anything is published ([publication approval](../../evidence-storage.md#package-publish-and-verify)).

## Not covered

- Git history before this commit still holds the earlier bytes of the 761 configs and the other tracked files. The 2026-11-13 pass rewrote the
  history and re-created the repository; this one did not.
- The rules find only these forms. A skill name or instruction text written in another form passes.
- `bench/runs/2026-09-29-codex-sol61-high-clean/transcripts.json` holds one `missing` entry whose frozen hash matches neither package. It was like
  that before this change.

## Verification

- `cargo test --lib redaction`: 23 passed, including four new tests for these rules.
- `bench-tools redaction check --require-values` on the tracked tree and on the redacted copy of every evidence package: 0 findings.
- A phrase scan for the instruction text, the skill names and the Windows account over the redacted copy: no matches.
