#!/usr/bin/env bash
# Run pinned grading queues one after another without an assistant session, and report on them from saved files.
#
#   bench/tools/regrade_queues.sh run LOGS WORKERS AUTHORIZATION=QUEUE_DIRECTORY...
#   bench/tools/regrade_queues.sh status QUEUE_DIRECTORY...
#
# `run` starts `bench-tools regrade run` for each queue in the order given.
# A queue that exits 5 has finished with blocked batches. The driver logs this and starts the next queue.
# After all queues finish, the driver exits 5 if any queue returned 5.
# Any other nonzero exit stops the driver and takes precedence over exit 5.
# Each controller run gets its own log under LOGS. LOGS/driver.log records each queue's start and end.
# Set LIMIT=N to stop each queue after N mapped or blocked batches.
# Run again with more WORKERS to raise concurrency on the batches left.
# A rerun resumes every queue from its directory.
# Exit 4 means the queue stopped launching at the plan's quota. A rerun resumes it once the meters allow.
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
  result=0
  for pair in "$@"; do
    authorization=${pair%%=*}; queue=${pair#*=}
    [ "$authorization" != "$pair" ] && [ -f "$authorization" ] || { echo "not AUTHORIZATION=QUEUE_DIRECTORY: $pair" >&2; exit 2; }
    name=$(basename "$queue")
    count=$(find "$logs" -maxdepth 1 -name "$name-*.log" | wc -l)
    log=$logs/$name-$((count + 1)).log
    echo "$(date -u +%FT%TZ) start $queue workers=$workers${LIMIT:+ limit=$LIMIT} -> $log" >> "$logs/driver.log"
    bench/tools/bench-tools regrade run --authorization "$authorization" \
      --directory "$queue" --workers "$workers" ${LIMIT:+--limit "$LIMIT"} > "$log" 2>&1
    code=$?
    echo "exit=$code" >> "$log"
    echo "$(date -u +%FT%TZ) end $queue exit=$code" >> "$logs/driver.log"
    [ $code -eq 4 ] && echo "$(date -u +%FT%TZ) $queue stopped at the plan's quota; run again to resume" >> "$logs/driver.log"
    case "$code" in
      0) ;;
      5)
        result=5
        echo "$(date -u +%FT%TZ) $queue finished with blocked batches; continuing" >> "$logs/driver.log"
        ;;
      *) exit "$code" ;;
    esac
  done
  echo "$(date -u +%FT%TZ) all queues finished exit=$result" >> "$logs/driver.log"
  exit "$result"
  ;;
status)
  [ $# -ge 1 ] || { echo "usage: $0 status QUEUE_DIRECTORY..." >&2; exit 2; }
  date -u +"now %FT%TZ"
  bench/tools/bench-tools regrade status -- "$@"
  ;;
*)
  sed -n '2,/^set -u/{ /^set -u/d; p; }' "$0" | sed 's/^# \{0,1\}//' >&2
  exit 2
  ;;
esac
