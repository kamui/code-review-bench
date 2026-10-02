# CE output interpretation

Preserve the complete `review.json` and all CE run artifacts as raw evidence. For common finding scoring, use only the top-level `findings` array. `actionable_findings` is a subset of that array and must not be counted a second time. Keep `pre_existing_findings` outside the primary finding set. Do not convert coverage, learnings, agent-native gaps, deployment notes, residual risks, testing gaps, or triage groups into primary findings.

Map each primary finding to the common item fields as follows:

- `file` <- `file`
- `line_start` / `line_end` <- the primary `line` value (if the native schema supplies only one line, set both to it); if absent, retain null rather than infer from prose
- `claim` <- `title`
- `consequence` <- `why_it_matters`
- `proposed_fix` <- `suggested_fix`
- `native_priority` <- `severity`
- `native_action` <- `autofix_class`
- `native_confidence` <- `confidence`
- `kind` <- `finding`

Retain stable finding `#`, `evidence`, `first_evidence`, `reviewers`, and `independent_reviewers` in the raw native JSON. Native verdict and status should remain available as adjunct fields in the normalized record. A malformed or missing `review.json` is unresolved; an explicit complete report with an empty `findings` array is empty.
