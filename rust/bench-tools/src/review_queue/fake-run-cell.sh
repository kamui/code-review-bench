#!/bin/sh
# A fake `bench-tools run-cell` for the queue tests: it claims, reviews and files by the files under $FAKE_ROOT.
set -eu
root=$FAKE_ROOT
printf '%s\n' "$*" >> "$root/calls.log"
run=$2
work=$4
shift 4
take() {
  if [ -e "$root/$1" ]; then cat "$root/$1"; rm -f "$root/$1"; fi
}
claim() {
  n=1
  while [ -e "$run/attempts/$(printf 'att-%03d' "$n")" ] || [ -e "$work/$(printf 'att-%03d' "$n")" ]; do
    n=$((n + 1))
  done
  attempt=$(printf 'att-%03d' "$n")
  target=${1%%/*}
  rest=${1#*/}
  arm=${rest%%/*}
  replicate=${rest#*/}
  mkdir -p "$work/$attempt"
  cell="{\"target\": \"$target\", \"arm\": \"$arm\", \"replicate\": $replicate}"
  printf '%s' "$1" > "$work/$attempt/key.txt"
  printf '%s' "$cell" > "$work/$attempt/cell.txt"
  printf '%s' "$2" > "$work/$attempt/predecessor.txt"
  printf '{"cell": %s, "reserved_usd": 5.0, "predecessor": %s, "queue_launch_marker": "%s"}' \
    "$cell" "$2" "${BENCH_QUEUE_LAUNCH:-}" > "$work/$attempt/cell.json"
}
file() {
  if [ -e "$run/attempts/$1/attempt.json" ]; then
    echo "$1 is already filed" >&2
    exit 3
  fi
  mkdir -p "$run/attempts/$1"
  printf '{"cell": %s, "disposition": "%s", "predecessor": %s, "usage": {"priced_total_usd": %s}}' \
    "$(cat "$work/$1/cell.txt")" "$2" "$(cat "$work/$1/predecessor.txt")" "$3" > "$run/attempts/$1/attempt.json"
  printf '%s\n' "$1" >> "$root/filed.log"
  if [ "$2" = "valid completed" ]; then
    printf '{"applied": true}' > "$work/$1/workspace-pruned.json"
  fi
}
review() {
  env | grep -E '^(BENCH_|CLAUDE|ANTHROPIC|AI_AGENT|QUEUE_)' | sort > "$root/env.log" || true
  crash=$(take crash)
  if [ "$crash" = before_review ]; then exit 9; fi
  printf '{}' > "$work/$attempt/timing.json"
  if [ -e "$root/survivor" ]; then
    rm -f "$root/survivor"
    (cd "$work/$attempt" && setsid sleep 300 < /dev/null > /dev/null 2>&1 & echo $! > "$root/survivor.pid")
  fi
  if [ "$crash" = during_review ]; then exit 9; fi
  if [ -e "$root/hold" ]; then
    rm -f "$root/hold"
    while [ ! -e "$root/release" ]; do sleep 0.02; done
  fi
  printf 'exit=0\n' > "$work/$attempt/dispatch.txt"
  if [ "$crash" = after_return ]; then exit 9; fi
  disposition=$(take disposition)
  usd=$(take usd)
  file "$attempt" "${disposition:-valid completed}" "${usd:-1.0}"
}
case $1 in
--cell)
  if [ "${3:-}" = --dry-run ]; then
    if [ -e "$root/refuse" ]; then cat "$root/refuse"; exit 1; fi
    exit 0
  fi
  claim "$2" null
  review
  ;;
--replace)
  claim "$(cat "$work/$2/key.txt")" "\"$2\""
  review
  ;;
--release)
  rm -rf "${work:?}/$2"
  ;;
--file)
  if [ "${3:-}" = --interrupted ]; then
    file "$2" "stopped: interrupted" null
  else
    disposition=$(take disposition)
    file "$2" "${disposition:-valid completed}" 1.0
  fi
  ;;
esac
