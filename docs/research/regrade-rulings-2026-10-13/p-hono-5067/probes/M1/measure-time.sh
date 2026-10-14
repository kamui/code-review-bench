#!/usr/bin/env bash
# Usage: measure-time.sh <label> <hono checkout> [processes per cell, default 3]
# Runs time.ts for every runtime, scenario and body size, each cell in fresh processes, then one counting pass.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
label="$1"; checkout="$2"; processes="${3:-3}"
echo "##### time.ts at $label: $(git -C "$checkout" rev-parse HEAD 2>/dev/null || echo "$label (release tarball)") $(git -C "$checkout" diff --stat 2>/dev/null | tail -1)"
echo "##### node $("${NODE_BIN:-node}" --version), bun $(bun --version); $(date -u +%Y-%m-%dT%H:%M:%SZ)"
cell() { # body file_bytes requests warmup
  for runtime in node bun; do
    for spec in "repeat 1" "repeat 2" "repeat 5" "validator 1"; do
      set -- $spec "$@"
      scenario="$1"; calls="$2"; shift 2
      for _ in $(seq "$processes"); do
        LABEL="$label" SCENARIO="$scenario" CALLS="$calls" BODY="$1" FILE_BYTES="$2" REQUESTS="$3" WARMUP="$4" \
          "$here/run.sh" "$here/time.ts" "$checkout" "$runtime"
      done
    done
  done
}
cell urlencoded-small 0 20000 2000
cell multipart-small 0 20000 2000
cell multipart-file 1048576 100 5
cell multipart-file 8388608 30 3
cell multipart-file 104857600 5 1
echo "##### counting pass (COUNT=1): platform form parses per request"
for runtime in node bun; do
  for spec in "repeat 1" "repeat 2" "repeat 5" "validator 1"; do
    set -- $spec
    for body in urlencoded-small multipart-file; do
      COUNT=1 LABEL="$label" SCENARIO="$1" CALLS="$2" BODY="$body" FILE_BYTES=1048576 REQUESTS=10 WARMUP=1 \
        "$here/run.sh" "$here/time.ts" "$checkout" "$runtime"
    done
  done
done
