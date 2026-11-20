# Owner ruling: redaction of a pinned evidence file

Recorded at 2026-11-20. Authority: user.

The owner approved one use of `bench-tools redaction repin`, described in [redact a pinned evidence file](../../../docs/evidence-storage.md#redact-a-pinned-evidence-file). The file is the results record of the Vercel run for Astro 16079. It names the Vercel account and the Vercel team the run used. A claim's evidence source and an eligibility decision's evidence pin name the file by hash, so `redaction apply` could not be used on it.

## What the owner was shown

The line the first run of `repin` printed, and what its two hashes are: the first is the file as committed on `main`; the second is that file with only the Vercel account name and the Vercel team name replaced by `[redacted-account]`, three values on two lines, with no other byte changed.

The owner was also told what the second run changes: the results file, one hash in `bench/grading/current/claims.json`, one hash in `bench/grading/current/adjudications.json`, and a new entry in `bench/grading/current/redacted-evidence.json`. `grades.json`, the assessment receipts and the audit records stay byte-identical, and no batch is graded again.

## The question

Asked on 2026-11-20:

> Do you approve redacting this pinned evidence file to exactly those bytes, using `bench-tools redaction repin`?

Options shown:

- "Approve (recommended)": The answer is saved as this ruling with the approval line. The second run of `repin` follows, then the grading check, then a pull request.
- "Don't approve": The file stays as it is, and the whole-tree redaction check keeps reporting it as the one known exception.

The owner answered:

> Approve

## The approval

Redaction approved for `docs/research/candidate-rulings-2026-11-07/o-astro-16079/vercel-run/RESULTS.md` from sha256 `e12ce71b6a7f6d9ffca52683f3d164efa38b39c05a67083c78f5eb30b1074a1d` to sha256 `b214f41615511e7765f19f032f8195d35842ee1dec974c096607e3389e72c614`
