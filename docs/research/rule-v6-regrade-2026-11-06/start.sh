#!/bin/sh
# Start the rule-version-6 regrade's four queues, detached, with Claude Code 2.1.317 first on PATH. mise removed its
# 2.1.317 install on 2026-11-06; the copy here is the rebuilt queue's pinned client-bin/claude (sha256 eb224419...).
# The lock keeps one driver however it is started; a second start exits 0. A rerun resumes every queue.
set -eu
export HOME=/home/t3
export BENCH_RUNTIME_STORE=/home/t3/.t3/bench-runtime
export PATH=/home/t3/.local/bin:/home/t3/.local/share/mise/installs/bun/1.4.4/bin:/home/t3/.cache/code-review-bench/claude-2.1.317:/home/t3/.local/share/mise/installs/go/1.27.3/bin:/home/t3/.cargo/bin:/usr/local/bin:/usr/bin:/bin
cd /home/t3/.t3/worktrees/code-review-bench/t3-aecb23e8
D=docs/research/rule-v6-regrade-2026-11-06
mkdir -p .local/regrade-v6/logs
exec /usr/bin/flock -n -E 0 /home/t3/.cache/regrade-v6.lock \
  bench/tools/regrade_queues.sh run .local/regrade-v6/logs 2 \
  $D/authorization.rebuilt.v1.json=.local/regrade-v6/rebuilt \
  $D/authorization.selected.v1.json=.local/regrade-v6/selected \
  $D/authorization.plain.v1.json=.local/regrade-v6/plain \
  $D/authorization.trpc.v1.json=.local/regrade-v6/trpc
