#!/bin/sh
# Run more of the node regrade's queues beside start.sh's driver. The owner asked "Can we parallelize more? I have a lot
# of claude usage left for the next ~3hours" and then "if you really think it's safe, then yes run them in parallel".
#   sh start-more.sh NAME WORKERS QUEUE...
# Each driver has its own log directory and lock; each queue's controller.lock keeps one controller per queue, and
# bench-tools serializes writes to the current record.
set -eu
name=$1; workers=$2; shift 2
export HOME=/home/t3
export BENCH_RUNTIME_STORE=/home/t3/.t3/bench-runtime
export PATH=/home/t3/.cache/code-review-bench/node-24.22.2/bin:/home/t3/.local/bin:/home/t3/.local/share/mise/installs/bun/1.4.4/bin:/home/t3/.cache/code-review-bench/claude-2.1.317:/home/t3/.local/share/mise/installs/go/1.27.3/bin:/home/t3/.cargo/bin:/usr/local/bin:/usr/bin:/bin
cd /home/t3/.t3/worktrees/code-review-bench/t3-aecb23e8
D=docs/research/node-regrade-2026-11-07
pairs=""
for queue in "$@"; do pairs="$pairs $D/authorization.$queue.v1.json=.local/node-regrade/$queue"; done
mkdir -p .local/node-regrade/logs-$name
exec /usr/bin/flock -n -E 0 /home/t3/.cache/node-regrade-$name.lock \
  bench/tools/regrade_queues.sh run .local/node-regrade/logs-$name "$workers" $pairs
