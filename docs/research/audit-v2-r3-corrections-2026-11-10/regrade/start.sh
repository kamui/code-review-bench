#!/bin/sh
# Start the audit round 3 corrections regrade, detached, with 3 workers. This is
# ../../candidate-regrade-2026-11-08/start.sh with this regrade's paths. Claude Code 2.1.317 comes first on PATH, as there.
# The lock keeps one driver; a rerun resumes the queue. It runs every authorization authorize.py wrote here.
set -eu
export HOME=/home/t3
export BENCH_RUNTIME_STORE=/home/t3/.t3/bench-runtime
export PATH=/home/t3/.local/bin:/home/t3/.local/share/mise/installs/bun/1.4.4/bin:/home/t3/.cache/code-review-bench/claude-2.1.317:/home/t3/.local/share/mise/installs/go/1.27.3/bin:/home/t3/.cargo/bin:/usr/local/bin:/usr/bin:/bin
cd /home/t3/.t3/worktrees/code-review-bench/t3-aecb23e8
D=docs/research/audit-v2-r3-corrections-2026-11-10/regrade
mkdir -p .local/audit-r3-corrections-regrade/logs
set --
for authorization in "$D"/authorization.*.v1.json; do
    name=${authorization#"$D"/authorization.}
    name=${name%.v1.json}
    set -- "$@" "$authorization=.local/audit-r3-corrections-regrade/$name"
done
[ "$#" -gt 0 ] || { echo "no authorization in $D; run authorize.py first" >&2; exit 1; }
exec /usr/bin/flock -n -E 0 /home/t3/.cache/audit-r3-corrections-regrade.lock \
  bench/tools/regrade_queues.sh run .local/audit-r3-corrections-regrade/logs 3 "$@"
