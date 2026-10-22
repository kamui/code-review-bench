#!/bin/sh
# One fake-client cell through the maintained path: dispatch.sh runs a stand-in Claude client under a fresh home, the
# run's pinned bench-tools audits and normalizes the review, and run-cell files it through bench/tools/bench-tools with
# the run's runtimes.frozen.json pin. No model, network or paid client is called. Linux only, as dispatch.sh is.
# Usage: tools/fake-client-cell.sh SCRATCH
# SCRATCH must not exist and must not be under /tmp. Prints the filed attempt; exits nonzero unless it is valid.
set -eu
checkout=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd -P)
root=$1
mkdir "$root"
root=$(cd "$root" && pwd -P)
launch() { "$checkout/bench/tools/bench-tools" "$@"; }
run_id=2026-01-01-python-free
run=$root/bench/runs/$run_id
work=$root/work/$run_id
attempt=$work/att-001
mkdir -p "$run/fixture" "$root/bench/arms" "$attempt" "$root/client" "$root/source/.claude"
ln -s "$checkout/bench/tools" "$root/bench/tools"
ln -s "$checkout/bench/harness" "$root/bench/harness"
ln -s "$checkout/bench/policies" "$root/bench/policies"

prompt='`high effort variant`

Review the diff.'
prompt_sha=$(printf '%s' "$prompt" | sha256sum | cut -d' ' -f1)
cat > "$root/bench/arms/arm-claude.json" <<EOF
{"id": "arm-claude", "kind": "claude-builtin", "model": "m-1", "effort": "high",
 "adapter": {"expected_prompt_variants": ["$prompt_sha"]}}
EOF
cat > "$run/manifest.json" <<EOF
{"run_id": "$run_id", "frozen_at": "2026-01-01T00:00:00Z",
 "arms": [{"id": "arm-claude", "expected_cli_version": "9.9.9", "resolved_skill_tree": null}],
 "planned_cells": [{"target": "t-fixture", "arm": "arm-claude", "replicate": 1}],
 "sealed_order": ["t-fixture/arm-claude/1"],
 "caps": {"max_attempts": 4, "replacements": 2, "spend_usd": 50.0, "closeout_reserve_usd": 0.0,
          "max_in_flight": 1, "attempt_usd": 5.0}}
EOF
launch runtime pin > "$run/runtimes.frozen.json"

clone=$attempt/clone
git init -q -b main "$clone"
dated() { GIT_AUTHOR_DATE=2026-01-01T00:00:00Z GIT_COMMITTER_DATE=2026-01-01T00:00:00Z \
    git -C "$clone" -c user.name=Fixture -c user.email=fixture@example.invalid "$@"; }
dated commit --allow-empty -qm base
dated checkout -q -b review-head
dated commit --allow-empty -qm head
printf '{"id": "t-fixture", "head": "%s", "merge_base": "%s"}\n' \
    "$(git -C "$clone" rev-parse review-head)" "$(git -C "$clone" rev-parse main)" > "$run/fixture/target.json"
cat > "$attempt/cell.json" <<'EOF'
{"cell": {"target": "t-fixture", "arm": "arm-claude", "replicate": 1}, "reserved_usd": 5.0, "predecessor": null,
 "retry_reason": null, "replacement_index": null, "billing_mode": "subscription"}
EOF
cat > "$attempt/rates.json" <<'EOF'
{"rates": [{"model": "m-1", "as_of": "2026-01-01", "input": 2.0, "output": 10.0, "cache_read": 0.2,
            "cache_write_5m": 2.5, "cache_write_1h": 4.0, "billing": "api-dollars"}]}
EOF
printf 'Review the fixture.\n' > "$root/packet.md"
printf '{}\n' > "$root/source/.claude/.credentials.json"
printf '{}\n' > "$root/source/.claude.json"

cat > "$root/client/claude" <<'EOF'
#!/bin/sh
if [ "${1-}" = --version ]; then echo "9.9.9 (Claude Code)"; exit 0; fi
session=
previous=
for argument in "$@"; do
    [ "$previous" = --session-id ] && session=$argument
    previous=$argument
done
project=$HOME/.claude/projects/fixture
mkdir -p "$project/$session/subagents"
cat > "$project/$session.jsonl" <<RECORD
{"type": "cost-state", "sessionId": "$session", "modelUsage": {"m-1": {"inputTokens": 10, "cacheCreationInputTokens": 100, "cacheReadInputTokens": 1000, "outputTokens": 50}}}
RECORD
cat > "$project/$session/subagents/agent-a1.jsonl" <<'RECORD'
{"type": "user", "message": {"content": "Review target: `main...review-head high`\n\n`high effort variant`\n\nReview the diff.\n"}}
{"type": "assistant", "requestId": "r1", "effort": "high", "timestamp": "2026-01-01T00:00:01Z", "message": {"model": "m-1", "stop_reason": "end_turn", "content": [{"type": "text", "text": "```json\n[]\n```"}], "usage": {"input_tokens": 10, "cache_creation_input_tokens": 100, "cache_read_input_tokens": 1000, "cache_creation": {"ephemeral_5m_input_tokens": 100, "ephemeral_1h_input_tokens": 0}, "output_tokens": 50}}}
RECORD
printf '%s\n' '{"type": "system"}' '{"type": "result", "subtype": "success", "result": "```json\n[]\n```"}'
EOF
chmod +x "$root/client/claude"

pinned=$(launch runtime resolve --run "$run" | sed -n 's/^ *"executable": "\(.*\)"$/\1/p')
HOME=$root/source BENCH_CLAUDE=$root/client/claude \
    BENCH_RUST_BENCH_TOOLS=$pinned BENCH_RUST_BENCH_TOOLS_SHA256=$(sha256sum "$pinned" | cut -d' ' -f1) \
    "$checkout/bench/tools/dispatch.sh" claude-builtin "$attempt" "$clone" main "$root/packet.md" m-1 high
BENCH_ARCHIVE_ROOT=$root/archive launch --run "$run" -- run-cell --run "$run" --work "$work" --file att-001
grep -q '"disposition": "valid completed"' "$run/attempts/att-001/attempt.json" || {
    echo "the fake-client attempt was not filed valid" >&2; exit 1; }
