# Pilot boundary and compatibility

`rust/usage-pilot` implements only the Codex `read_rollout`/`summarize` boundary.
`tools/usage_pilot/adapter.py` reuses the existing Python CLI for discovery and
presentation. There is no new production entrypoint or runtime fallback.

The internal executable accepts a path, four rates, an optional repeat count and
an optional warmup count. Each success emits one JSON summary per measured read.
Stderr carries loop measurements on success or a structured `{kind, detail}` error
on failure. It exits 2 on input, arithmetic or I/O failure. The adapter translates
errors into the existing CLI's diagnostics and preserves its no-usage exit 1.
The executable is an internal comparison tool, not a replacement public CLI.

All five Codex counts are required for retained requests. Unknown values survive
deduplication until summarization; a later complete record can replace an earlier
unknown record. A malformed count is rejected when read, even if a later record
would replace it. Last-record deduplication retains first tool attribution and the
baseline's pending-tool behavior. The same response ID in separate files stays
separate. Ordinal fallback and real response IDs are distinct variants, fixing the
baseline collision with a real `ordinal-None` ID. Arbitrarily large integer ordinals
are preserved. Counts themselves are limited to unsigned 64-bit integers.

Both candidates reject negative, fractional, boolean or out-of-range counts,
non-object records, non-object payloads on the four consumed record types, invalid
identities, mixed naive/aware timestamps,
overflowing totals and nonfinite rates/costs. These are deliberate pilot boundary
checks, rather than a claim that the permissive baseline accepts exactly the same
input language. Cache reads plus writes cannot exceed Codex input. Thinking counts
retain the baseline interpretation; this pilot adds no new output-subset rule.
The adapter also checks totals across input files. Other record types are ignored
without validating their payload, matching the baseline.

The Rust parser accepts extended RFC 3339, basic ISO date/time with zone, naive
extended/basic date/time, and date-only stamps. It rejects leap seconds and years
outside Python's 1–9999 range as unparseable. Missing/unparseable stamps leave timing
unknown. Python's `fromisoformat` accepts additional forms such as ISO week dates and
minute-only times; those are outside the native timestamp forms validated here.
Conversely, Rust accepts lowercase `t`/`z` and unpadded forms such as
`2026-10-18t10:00:07z` and `2026-1-1T0:0:5Z` that both Python variants leave
unparseable; this can produce different `wall_seconds`. Rust splits JSONL at LF
and accepts CRLF through JSON whitespace handling. Python's universal-newline
reader also splits at CR alone; multiple records separated only by CR therefore
parse in Python but fail as malformed JSON in Rust. These differences are accepted
as limits of this isolated pilot, not production compatibility decisions. Normalizing
these forms is deferred unless a separately reviewed integration needs them.
Rust uses serde_json's strict JSON and default nesting limit; Python's JSON decoder
has different nonfinite-number and depth behavior. These compatibility limits
preclude an unqualified public-parser parity or cutover claim. The unchanged corpus
and generated tests establish equivalence for their named input classes only.

For transcripts, both candidate maps use the same Python prevalidation and existing
calculation. Required input/output/cache counts and complete JSONL are validated
before any total is rendered. Unknown cache-write tier partitions and low/high cost
bounds remain intact. This resolves U2's compatibility decision by rejecting damage
instead of producing complete-looking partial accounting. Transcript internals,
optional fields and aggregation have not been ported to the Rust domain. No Rust
transcript performance or structural guarantee is claimed.

The application forbids unsafe Rust; dependencies retain their own implementation
policies. Private Rust fields restrict count, rate and identity construction through
the public safe API. Summary arithmetic and costs still require runtime checks;
the types cannot validate billing truth, file provenance, or external side effects. Production still has the separately recorded U1–U3 defects.
