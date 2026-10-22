# Usage baseline defect register

These independent expectations remain authoritative when Python differs.
Each entry owns a distinct resolution in [#152](https://github.com/kamui/code-review-bench/issues/152).
The pilot must implement the expectation or obtain an explicit compatibility decision
before expansion. Updating expected output to match Python does not resolve a defect.

## Pilot resolution in #152

The isolated [pilot](../pilot/protocol.md) adopts all three independent expectations.
Its typed Python and Rust Codex adapters reject U1; their shared Python transcript
adapter rejects U2 and U3 before the existing transcript calculation. This records
the compatibility decision for U2's previously documented skip behavior: a damaged
transcript cannot produce complete accounting. These diagnostics apply to the pilot
only. Production callers retain their current implementation and the baseline defects
below remain reproducible there. The corpus and its saved observations are unchanged.

## U1: Missing counts become zero

- Reproduced at baseline `5f188bf7c3aaf479f7dbd577f6f45d417f8b26b6` by
  `unknown-is-not-zero` in `tools/behavior_contracts/usage.v1.json`.
- Input is a `token_usage_record` with an empty usage object and a response ID.
- Independent expectation: exit 2, no stdout, and a missing `input_tokens` diagnostic.
  Missing accounting cannot establish known zero under #149's invariant.
- Observed: exit 0, one turn, zero input and output, and zero cost. `summarize`
  in `bench/tools/codex_usage.py` uses `usage.get(...) or 0` for missing fields.
- Resolution task U1 for #152: define required token categories and reject absent or
  null required counts before arithmetic in both pilot implementations. Preserve
  explicitly known zero and save a compatibility decision for changed diagnostics.
- Status: open, production unchanged by #151. The exact observed result is retained
  separately under `baseline_defect.observed`; it does not replace `expected`.

The [corpus runner](README.md#runner-moved)'s `self-check::open-defects` checks that this recorded
baseline result still fails the independent expectation. No runner mode accepts it as a pass.

## U2: Transcript truncation is accepted

- Reproduced at baseline `5f188bf7c3aaf479f7dbd577f6f45d417f8b26b6` by
  `transcript-truncated` in `tools/behavior_contracts/usage.v1.json`.
- A valid billed assistant request is followed by a partial JSON line.
- Independent expectation: exit 2, empty stdout, and a diagnostic naming line 2.
  A damaged log cannot establish complete accounting.
- Observed: exit 0, one turn, input 5, output 1, cost 0.000020 dollars. The current
  `read_transcript` API explicitly documents skipping invalid JSON lines.
- Resolution task U2 for #152: decide compatibility for this documented behavior
  before porting transcript usage. Implement rejection, or save an explicit decision
  for a representation that preserves incomplete accounting. A partial total must
  not silently become complete metering in `contributor_adapters.py`.
- Status: open. Production behavior is unchanged; the observed full JSON and effects
  are saved separately from the independent rejection expectation.

## U3: Transcript missing counts become zero

- Reproduced at baseline `5f188bf7c3aaf479f7dbd577f6f45d417f8b26b6` by
  `transcript-unknown` in `tools/behavior_contracts/usage.v1.json`.
- An assistant request has an empty usage object.
- Independent expectation: exit 2, empty stdout, and a missing `input_tokens` diagnostic.
- Observed: exit 0, one turn, zero token counts, and cost 0. `read_transcript` uses
  `used.get(...) or 0` and therefore loses the unknown state, as U1 does for Codex.
- Resolution task U3 for #152: validate required counts before aggregation and preserve
  known zero separately from absent or null accounting. Track this independently of U1
  because the transcript parser and released diagnostics have separate callers.
- Status: open, production unchanged. The fixture retains the observed full JSON and
  effects separately from its independent expectation.
