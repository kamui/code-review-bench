# Usage baseline defect register

These independent expectations remain authoritative when Python differs.
Each entry owns a distinct resolution in [#152](https://github.com/kamui/code-review-bench/issues/152).
The pilot must implement the expectation or obtain an explicit compatibility decision
before expansion. Updating expected output to match Python does not resolve a defect.

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

Run `python3 tools/behavior_contract.py` to see the actual mismatch. The explicit
`--allow-known-defects` mode recognizes only that recorded baseline result.
