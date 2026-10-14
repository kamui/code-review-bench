#!/usr/bin/env bash
# Usage: measure-memory.sh <label> <hono checkout>
# Runs memory.ts three times per runtime, scenario, number of calls and upload size, each run in a fresh process,
# one process at a time.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
label="$1"; checkout="$2"
echo "##### memory.ts at $label: $(git -C "$checkout" rev-parse HEAD 2>/dev/null || echo "$label (release tarball)") $(git -C "$checkout" diff --stat 2>/dev/null | tail -1)"
echo "##### node $("${NODE_BIN:-node}" --version), bun $(bun --version); $(date -u +%Y-%m-%dT%H:%M:%SZ)"
for size in 1 8 100; do
  for runtime in node bun; do
    for spec in "repeat 1 1" "repeat 2 1" "repeat 5 1" "validator 1 1" "repeat 2 0" "repeat 5 0"; do
      set -- $spec
      echo
      echo "## runtime=$runtime scenario=$1 calls=$2 hold=$3 upload=${size}MB (3 fresh processes)"
      for _ in 1 2 3; do
        LABEL="$label" SCENARIO="$1" CALLS="$2" HOLD="$3" FILE_MB="$size" "$here/run.sh" "$here/memory.ts" "$checkout" "$runtime"
      done
    done
  done
done
