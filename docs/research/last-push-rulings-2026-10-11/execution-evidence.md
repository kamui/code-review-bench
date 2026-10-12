# Original execution evidence

The [published manifest](../../../bench/evidence/manifests/last-push-rulings-2026-10-11-v1.json) preserves 276 original files for all 29 sessions in `usage.v1.json`. The [execution index](execution-evidence.v1.json) pins the manifest, restoration receipt, publication record and usage file. Each session names its hash-pinned exec log and native transcript.

The package holds the intake's blinded input parts, prompts, opaque-id key, raw matches, logs and exit record; the selected assessors' cases, rules, answers, notes, execution script and logs; and all 29 native transcripts. Every exec-log hash matches the saved usage. Publication downloaded the uploaded asset and restored every member, then a separate cold verification repeated that check. The [restoration receipt](../../../bench/evidence/receipts/last-push-rulings-2026-10-11-v1-restoration.json) records the package and member hashes.

The [publication record](evidence-publication.v1.json) states the public destination, authorization, selection, credential review and exclusions. Native transcripts preserve the historical context and configuration that actually ran. Their presence supplies no retroactive proof of an empty benchmark harness. The usage remains a list-price equivalent for sessions run on the ChatGPT plan, not a payment receipt. All local originals and frozen before/after pairs remain unchanged.

## Retrieve the evidence

Run from the repository root:

```sh
python3 bench/tools/evidence_store.py fetch --manifest bench/evidence/manifests/last-push-rulings-2026-10-11-v1.json
python3 bench/tools/evidence_store.py verify --manifest bench/evidence/manifests/last-push-rulings-2026-10-11-v1.json
```

`capture_evidence.py` rebuilds the selection and session index from the original intake directory, assessor directory and native session root. It selects only sessions in `usage.v1.json`, verifies their saved log hashes and native thread identities, and writes into a new output directory. Repeating capture produced identical selection and index bytes before packaging.

## Declared confidence

The [declared-answer supplement](rulings/declared-answers.v1.json) pins the frozen K2 before-record and the original two assessor answer lists. Both assessors declared high confidence and `would_settle: true`. The before-record retained medium eligibility because its nearest rulings were of the earlier reading. `ruling_summary.py` uses the high declarations for calibration while consensus, routing and earned trust keep the normalized eligibility. No before/after pair or source answer was edited.
