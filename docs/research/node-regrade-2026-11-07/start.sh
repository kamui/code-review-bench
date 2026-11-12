#!/bin/sh
# Start the node regrade's four queues, detached, with 3 workers. A copy of /usr/local/bin/node sits outside /usr first
# on PATH, so the pinned bench-tools mounts it and puts its directory on the sandbox PATH (PR #442 fixes a node under
# /usr). Claude Code 2.1.317 is the rebuilt queue's pinned copy, as in ../rule-v6-regrade-2026-11-06/start.sh.
# The lock keeps one driver however it is started; a second start exits 0. A rerun resumes every queue.
set -eu
export HOME=/home/t3
export BENCH_RUNTIME_STORE=/home/t3/.t3/bench-runtime
export PATH=/home/t3/.cache/code-review-bench/node-24.22.2/bin:/home/t3/.local/bin:/home/t3/.local/share/mise/installs/bun/1.4.4/bin:/home/t3/.cache/code-review-bench/claude-2.1.317:/home/t3/.local/share/mise/installs/go/1.27.3/bin:/home/t3/.cargo/bin:/usr/local/bin:/usr/bin:/bin
cd /home/t3/.t3/worktrees/code-review-bench/t3-aecb23e8
D=docs/research/node-regrade-2026-11-07
mkdir -p .local/node-regrade/logs
exec /usr/bin/flock -n -E 0 /home/t3/.cache/node-regrade.lock \
  bench/tools/regrade_queues.sh run .local/node-regrade/logs 3 \
  $D/authorization.trpc.v1.json=.local/node-regrade/trpc \
  $D/authorization.rebuilt.v1.json=.local/node-regrade/rebuilt \
  $D/authorization.plain.v1.json=.local/node-regrade/plain \
  $D/authorization.selected.v1.json=.local/node-regrade/selected
