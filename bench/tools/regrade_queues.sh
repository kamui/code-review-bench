#!/usr/bin/env bash
# Run pinned grading queues one after another without an assistant session, and report on them from saved files.
#
#   bench/tools/regrade_queues.sh run LOGS WORKERS AUTHORIZATION=QUEUE_DIRECTORY...
#   bench/tools/regrade_queues.sh status QUEUE_DIRECTORY...
#
# `run` starts regrade.py for each queue in the order given and stops at the first queue that does not finish.
# Each controller run gets its own log under LOGS, and LOGS/driver.log gets one line when a queue starts and one
# when it ends. Set LIMIT=N to map at most N batches of each queue and stop: run again with more WORKERS to
# raise concurrency on the batches that are left. A rerun resumes every queue from its directory.
#
# `status` calls no model. It prints each queue's mapped and planned batches, state, list-price spend, sessions
# in flight, pinned client and the wall time of its runs, then the free space of the queue's filesystem.
#
# Start `run` detached from any terminal or assistant, for example:
#   nohup bench/tools/regrade_queues.sh run .local/regrade/logs 3 A.json=.local/regrade/a B.json=.local/regrade/b &
set -u
cd "$(dirname "$0")/../.." || exit 9
command=${1:-}
[ $# -gt 0 ] && shift

case "$command" in
run)
  [ $# -ge 3 ] || { echo "usage: $0 run LOGS WORKERS AUTHORIZATION=QUEUE_DIRECTORY..." >&2; exit 2; }
  logs=$1; workers=$2; shift 2
  mkdir -p "$logs"
  # The client reports its start directory to the model, and dispatch refuses one whose path names a run.
  export TMPDIR=${TMPDIR:-$HOME/.cache/code-review-bench/tmp}
  mkdir -p "$TMPDIR"
  for pair in "$@"; do
    authorization=${pair%%=*}; queue=${pair#*=}
    [ "$authorization" != "$pair" ] && [ -f "$authorization" ] || { echo "not AUTHORIZATION=QUEUE_DIRECTORY: $pair" >&2; exit 2; }
    name=$(basename "$queue")
    count=$(find "$logs" -maxdepth 1 -name "$name-*.log" | wc -l)
    log=$logs/$name-$((count + 1)).log
    echo "$(date -u +%FT%TZ) start $queue workers=$workers${LIMIT:+ limit=$LIMIT} -> $log" >> "$logs/driver.log"
    python3 bench/tools/regrade.py --authorization "$authorization" --directory "$queue" --workers "$workers" \
      ${LIMIT:+--limit "$LIMIT"} > "$log" 2>&1
    code=$?
    echo "exit=$code" >> "$log"
    echo "$(date -u +%FT%TZ) end $queue exit=$code" >> "$logs/driver.log"
    [ $code -eq 0 ] || exit $code
  done
  echo "$(date -u +%FT%TZ) all queues finished" >> "$logs/driver.log"
  ;;
status)
  [ $# -ge 1 ] || { echo "usage: $0 status QUEUE_DIRECTORY..." >&2; exit 2; }
  date -u +"now %FT%TZ"
  python3 - "$@" <<'PYTHON'
import json, shutil, sys
from pathlib import Path

for queue in map(Path, sys.argv[1:]):
    if not (queue / "status.json").exists():
        print(f"{queue}: not started")
        continue
    status = json.loads((queue / "status.json").read_text())
    mapped = sum(row["state"] == "mapped" for row in status["batches"])
    other = sorted({row["state"] for row in status["batches"]} - {"mapped", "pending"})
    runs = status["invocations"]
    client = json.loads((queue / "client.json").read_text())["version"] if (queue / "client.json").exists() else "not pinned"
    spent = "unknown" if status["spentUpperUsd"] is None else f"${status['spentUpperUsd']:.2f}"
    print(f"{queue}: mapped {mapped} of {status['plannedBatches']} | {status['state']}"
          + (f" ({status['reason']})" if status["reason"] else "") + (f" | also {', '.join(other)}" if other else "")
          + f" | spent {spent} of ${status['budgetCapUsd']} at list price | in flight {status['outstandingReservations']}"
          + f" | client {client} | {len(runs)} run(s), {sum(run.get('wallSeconds', 0) for run in runs) / 3600:.2f} h,"
          + f" workers {'/'.join(str(run['workers']) for run in runs)}")
    print(f"    free space: {shutil.disk_usage(queue).free / 2**30:.0f} GiB")
PYTHON
  ;;
*)
  sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//' >&2
  exit 2
  ;;
esac
