#!/bin/sh
# Start the follow-on candidate regrade's one queue, rebuilt (all 18 batches), detached, with 3 workers. The installed bench-tools puts a node under /usr/
# on the sandbox PATH (PR #442), so no copy of node is needed. Claude Code 2.1.317 is the rebuilt queue's pinned copy, as
# in ../rule-v6-regrade-2026-11-06/start.sh. The lock keeps one driver; a rerun resumes the queue.
set -eu
export HOME=/home/t3
export BENCH_RUNTIME_STORE=/home/t3/.t3/bench-runtime
export PATH=/home/t3/.local/bin:/home/t3/.local/share/mise/installs/bun/1.4.4/bin:/home/t3/.cache/code-review-bench/claude-2.1.317:/home/t3/.local/share/mise/installs/go/1.27.3/bin:/home/t3/.cargo/bin:/usr/local/bin:/usr/bin:/bin
cd /home/t3/.t3/worktrees/code-review-bench/t3-aecb23e8
D=docs/research/candidate-regrade-2026-11-08
mkdir -p .local/candidate-regrade-2/logs
exec /usr/bin/flock -n -E 0 /home/t3/.cache/candidate-regrade-2.lock \
  bench/tools/regrade_queues.sh run .local/candidate-regrade-2/logs 3 \
  $D/authorization.rebuilt.v1.json=.local/candidate-regrade-2/rebuilt
