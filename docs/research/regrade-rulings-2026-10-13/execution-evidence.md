# Original execution evidence

The [published manifest](../../../bench/evidence/manifests/regrade-rulings-2026-10-13-v1.json) preserves 474 original files for the 47 blind sessions in `usage.v1.json` and the one review session in `filing/review.v1.json`. The [execution index](execution-evidence.v1.json) pins the manifest, the restoration receipt, the publication record and both usage files. Each session names its hash-pinned exec log and native transcript.

For each blind session the package holds its working folder as the session left it: the cases, the rule, the sheet of earlier rulings, and the answers and notes it wrote. It holds the session's `codex exec --json` log, error output and last message, its native transcript, and the four scripts that built the folders and ran the sessions. For the review of the filing it holds the prompt, the log, the exit record, the last message and the native transcript. Every exec-log hash matches the saved usage. Publication downloaded the uploaded asset and restored every member, then a separate cold verification repeated that check. The [restoration receipt](../../../bench/evidence/receipts/regrade-rulings-2026-10-13-v1-restoration.json) records the hashes of the manifest and the package; the manifest holds each member's hash.

The package keeps the three sessions that were run again. `i-credit-astra` stopped on a sandbox error and wrote no answer; `i-credit-astra-2` is its rerun. `u-credit-sol` and `u-credit-astra` read the case file `C1.md` before it was corrected. Their folders hold that earlier file, which is not among the round's tracked files, and `u-credit-sol-2` and `u-credit-astra-2` read the corrected one.

The [publication record](evidence-publication.v1.json) states the public destination, authorization, selection, credential review and exclusions. The owner approved the publication on 2026-10-13. The session that received the owner's answers wrote the authorization into the record and published the package, and the record holds the questions and the answers.

## What the evidence does not show

The sessions ran in the account's own Codex home, not in an empty harness. Each native transcript keeps the context and configuration that ran: the model, effort, sandbox and approval policy of the turn, the base instructions, the account's own instructions file and its list of installed skills. The account's `config.toml` is not in the package, because it holds settings for unrelated projects. Ten blind sessions and the review session read one file outside their folder, the account's `unslop` writing skill, which the account's instructions require for any writing. The transcripts show this as it happened and supply no proof of an empty benchmark harness.

The usage is a list-price equivalent for sessions run on the ChatGPT plan, not a payment receipt. The preparation agents, the checkers of the case files and the asking session are not in `usage.v1.json`, which lists the Codex sessions only; none of their transcripts is in the package. All local originals remain unchanged.

## Retrieve the evidence

Run from the repository root:

```sh
python3 bench/tools/evidence_store.py fetch --manifest bench/evidence/manifests/regrade-rulings-2026-10-13-v1.json
python3 bench/tools/evidence_store.py verify --manifest bench/evidence/manifests/regrade-rulings-2026-10-13-v1.json
```

The files restore under `artifacts/research/regrade-rulings-2026-10-13/`: `assessors/<session>/` for a working folder, `assessors/logs/` for the exec logs, `native/<session>.jsonl` for a native transcript and `filing-review/` for the review of the filing.

`capture_evidence.py` rebuilds the selection and session index from the assessor directory, the review session's folder and the native session root. It selects only the sessions in `usage.v1.json` and `filing/review.v1.json`, verifies their saved log hashes and native thread identities, and writes into a new output directory. Repeating capture produced identical selection and index bytes before packaging.
