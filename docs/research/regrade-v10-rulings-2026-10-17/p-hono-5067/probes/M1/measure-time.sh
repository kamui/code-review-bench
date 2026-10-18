#!/usr/bin/env bash
# Usage: measure-time.sh <scratch directory holding one checkout per variant> > results/time-main.txt
# Times the first c.req.parseBody() of a request (time.ts) for every runtime, body and size. Each line of output is one
# fresh process. Within a cell the variants run in turn, round after round, so that a change in the machine's load
# falls on every variant alike. base and head run 5 rounds in both body modes; each patch runs 3 rounds, streamed only.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
scratch="$1"
patches="fix-reuse fix-stream-both fix-reuse-and-stream fix-native-lower fix-release guard-only"
echo "##### time.ts main matrix; node $("${NODE_BIN:-node}" --version), bun $(bun --version); start $(date -u +%Y-%m-%dT%H:%M:%SZ); load $(cut -d' ' -f1-3 /proc/loadavg)"
for v in base head $patches; do
  echo "##### $v: $(git -C "$scratch/$v" rev-parse HEAD) $(git -C "$scratch/$v" diff --stat | tail -1)"
done
cell() { # runtime mode body file_mb requests warmup
  local runtime="$1" mode="$2" body="$3" mb="$4" requests="$5" warmup="$6"
  for round in 1 2 3 4 5; do
    variants="base head"
    if [ "$mode" = stream ] && [ "$round" -le 3 ]; then variants="base head $patches"; fi
    for v in $variants; do
      LABEL="$v" PATH_KIND=hono OP=parseBody MODE="$mode" BODY="$body" FILE_MB="$mb" CT=lower \
        REQUESTS="$requests" WARMUP="$warmup" "$here/run.sh" "$here/time.ts" "$scratch/$v" "$runtime"
    done
  done
}
for runtime in node bun; do
  for mode in stream bytes; do
    cell "$runtime" "$mode" urlencoded 0 20000 2000
    cell "$runtime" "$mode" multipart-small 0 20000 2000
    cell "$runtime" "$mode" file 1 100 5
    cell "$runtime" "$mode" file 8 40 4
    cell "$runtime" "$mode" file 32 15 2
    cell "$runtime" "$mode" file 100 6 1
    cell "$runtime" "$mode" file 128 5 1
  done
done
echo "##### end $(date -u +%Y-%m-%dT%H:%M:%SZ); load $(cut -d' ' -f1-3 /proc/loadavg)"
