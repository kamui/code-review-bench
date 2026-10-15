# Experimental Jev caller

Run from the repository root, with `TYPESAFE_API_KEY` already provided in the
environment. The runner never reads credential files or writes the key.

```sh
python3 docs/research/jev-grading-2026-10-15/runner/jev_runner.py \
  --manifest /absolute/path/to/frozen-manifest.json \
  --manifest-sha256 EXACT_SHA256_OF_MANIFEST_BYTES \
  --output /absolute/path/to/new-output-directory
```

The manifest contains only request references:

```json
{
  "schema_version": 1,
  "mode": "development",
  "requests": [
    {"id": "case-v1", "path": "requests/case-v1.json", "sha256": "REQUEST_BYTES_SHA256"}
  ]
}
```

Modes are `pilot`, `development`, and `qualification`. Request paths must remain
within the manifest's directory. Each request contains exactly `model`, `state`,
and `questions`. The model must be `jev-1.13.0`; each question contains exactly
`type`, `instructions`, and `criteria`. Choice uses `yes`, `no`, and
`cannot_tell`; Noul uses `true` and `false` and returns a probability.
Noul support was added for development v11/v12 and does not imply a calibrated
certainty threshold. Three-choice distributions allow up to 0.015 rounding error,
matching the observed two-decimal service outputs; the original rejected attempt
and its reservation remain preserved. Keep answer keys and evaluation annotations outside these
files. Additional top-level fields are refused; state construction remains the
dataset author's responsibility.

The fixed global ledger is `../spend-ledger/events.jsonl`, relative to this
directory. All modes share the $4.95 cap. An exclusive operating-system lock
covers the batch. Before dispatch, the runner reserves $0.005505024 for 131,072
input tokens. Completed, validated responses replace that reservation with exact
usage cost. HTTP, network, response-validation, and model-version failures retain
the reservation and stop the batch. A crash also retains the reservation.
Already attempted request hashes cannot be retried by a new manifest or output
directory. There is no retry or charge-release option.

Each attempt gets an exclusive directory and `clean_context.prepare` receipt.
It captures request and response bytes, hashes, timestamps, elapsed time, usage,
resolved model, and source hashes. `summary.json` contains results and global
accounting. A response echoing the credential is redacted and remains an unknown
charge. Exit codes are 0 for a fully completed batch, 1 for a captured failed
attempt, and 2 for refusal or a local failure. After a local failure, inspect the
ledger and per-attempt artifacts; existing reservations still count.

The [API reference](https://docs.typesafe.ai/api) and
[model reference](https://docs.typesafe.ai/models), checked 2026-10-15, specify
$0.042 per million input tokens, free output tokens, and default limits of 64k
tokens per request and 32k per question. This runner refuses over 60,000 UTF-8
request bytes or over 30,000 bytes for state plus one question, leaving headroom
for service framing. It sends no long-context header, follows no redirects,
disables proxy discovery, and makes one HTTP POST per attempt.

Run the offline tests:

```sh
python3 -m unittest discover -s docs/research/jev-grading-2026-10-15/runner -v
```
