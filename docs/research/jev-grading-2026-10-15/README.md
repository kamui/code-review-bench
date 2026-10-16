# Bounded grading experiments: Jev, Sol and Luna

These historical experiments tested whether a cheaper classifier could answer
bounded grading questions while an LLM handled investigations. Neither Jev nor
Luna qualified for automatic credit or investigation routing. Production grading
and the scorecard are unchanged. This is supporting research for [#30](https://github.com/kamui/code-review-bench/issues/30).

- [Jev experiment and Sol comparison](REPORT.md): twelve input formulations,
  saved human labels, confidence-routing results, failures and spend accounting.
- [Luna follow-up](../luna-grading-2026-10-15/REPORT.md): original and example
  inputs, corrected example instructions, output-contract experiments and usage.
- [Original plan](PLAN.md), [Jev decisions](decisions.tsv), and
  [Luna decisions](../luna-grading-2026-10-15/decisions.tsv) preserve the sequence.

## Storage and restoration

Git retains the frozen dataset, human reference labels, input variants, runtime
snapshots, scripts, reports and metrics. Raw responses, failed attempts, usage,
complete session JSONL, explicit client configuration and no-charge probes are
stored through the [evidence contract](../../evidence-storage.md). Generated
client databases, bundled skills, locks and executable links are excluded from
the package; the original local workspaces remain intact. See the
[selection record](evidence-selection.v1.json) and [publication scope](publication.v1.json).

The [published manifest](../../../bench/evidence/manifests/classifier-experiments-2026-10-15-v1.json)
pins every selected file in the [immutable release](https://github.com/kamui/code-review-bench/releases/tag/evidence-classifier-experiments-2026-10-15-v1).
The [remote restoration receipt](../../../bench/evidence/receipts/classifier-experiments-2026-10-15-v1-restoration.json)
records a fresh download and verification. Restore the payload with:

```sh
python3 bench/tools/evidence_store.py fetch \
  --manifest bench/evidence/manifests/classifier-experiments-2026-10-15-v1.json
```

Results restore to their original `docs/research/<experiment>/results/` paths.
Historical `.local/<experiment>/` probe paths map to
`docs/research/<experiment>/probes/`; frozen records are not rewritten. Dispatch
receipts retain originating absolute paths as provenance. Their session evidence
is restored alongside the receipt. `runtime-snapshots/` preserves each adapter
version named by those receipts; these snapshots are archival source, not
standalone entry points.

## Recompute without model calls

After restoring the payload, run from the repository root:

```sh
python3 docs/research/jev-grading-2026-10-15/summarize.py
python3 docs/research/luna-grading-2026-10-15/summarize.py
python3 -m unittest discover -s docs/research/jev-grading-2026-10-15/runner -q
python3 -m unittest discover -s docs/research/jev-grading-2026-10-15/codex -q
python3 -m unittest discover -s docs/research/jev-grading-2026-10-15 -p 'test_evaluate.py' -q
```

The summaries verify frozen dataset/input hashes and response/session artifacts
before calculating metrics. Human reference labels are evaluation-only, except
for the expressly frozen development examples. Never send the private key file
as a classifier packet.

`dataset/generate.py --check` additionally reconstructs the original dataset from
the live adjudication sources. It requires the original adjudication source bytes from revision
`1727db27263cb652af3d6fd1bf337cb1507950c8`; later human rulings can intentionally
change those sources. Recomputing the frozen summaries does not require a new
adjudication or a paid model call. The dispatch scripts are historical experiment
tools with dated quota and probe pins, not a newly authorized live grading job.

[Cold restoration checks](cold-check.v1.json) record recomputation in a separate
checkout populated from the prepared archive, without the original result paths.

The reports describe the end of data collection, before this publication. The
[publication checks](publication-verification.v1.json) cover packaging and replay.
Replaying with the final evaluator preserves the reported numbers but can add
validation metadata to older metric files; the original saved metrics remain in
Git. No new classifier calls were made for publication.
