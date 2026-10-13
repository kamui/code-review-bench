#!/usr/bin/env bash
# Usage: measure.sh <label> <hono checkout>
# Runs probe.ts three times per runtime, mode and upload size, each run in a fresh process, one process at a time,
# then one tracking pass (TRACK=1) per runtime and mode for the 100 MB upload.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
label="$1"; checkout="$2"
echo "##### probe.ts at $label: $(git -C "$checkout" rev-parse HEAD 2>/dev/null || echo "$label (release tarball)") $(git -C "$checkout" diff --stat 2>/dev/null | tail -1)"
echo "##### node $("${NODE_BIN:-node}" --version), bun $(bun --version); $(date -u +%Y-%m-%dT%H:%M:%SZ)"
for size in 8 50 100; do
  for runtime in node bun; do
    for mode in plain hono native; do
      echo
      echo "## runtime=$runtime mode=$mode upload=${size}MB (3 fresh processes)"
      for _ in 1 2 3; do
        LABEL="$label" MODE="$mode" FILE_MB="$size" "$here/../M1/run.sh" "$here/probe.ts" "$checkout" "$runtime"
      done
    done
  done
done
echo
echo "##### tracking pass (TRACK=1), 100 MB: is the buffer returned by Request.prototype.arrayBuffer still reachable after the parse?"
for runtime in node bun; do
  for mode in plain hono; do
    TRACK=1 LABEL="$label" MODE="$mode" FILE_MB=100 "$here/../M1/run.sh" "$here/probe.ts" "$checkout" "$runtime"
  done
done
