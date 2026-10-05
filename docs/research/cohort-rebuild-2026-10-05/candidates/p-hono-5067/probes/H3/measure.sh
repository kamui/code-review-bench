#!/usr/bin/env bash
# Usage: measure.sh <label> <hono checkout>
# Runs probes/H3/probe.ts three times per runtime, mode and upload size, each run in a fresh process.
# NODE_BIN pins the node binary so every checkout runs on the same node version.
set -euo pipefail
node_bin="${NODE_BIN:-node}"
here="$(cd "$(dirname "$0")" && pwd)"
label="$1"; checkout="$2"
mkdir -p "$checkout/_probe"
cp "$here/probe.ts" "$checkout/_probe/probe.ts"
cd "$checkout/_probe"
bun build probe.ts --target=node --outfile=probe.node.mjs >/dev/null
echo "##### H3 at $label: $(git -C "$checkout" rev-parse HEAD 2>/dev/null || echo "$label (tarball)"); node $("$node_bin" --version), bun $(bun --version)"
for mode in parseBody validator; do
  for size in 50 100; do
    for runtime in node bun; do
      echo
      echo "## runtime=$runtime mode=$mode upload=${size}MB (3 fresh processes)"
      for _ in 1 2 3; do
        if [ "$runtime" = node ]; then
          MODE=$mode SIZE_MB=$size "$node_bin" --expose-gc probe.node.mjs 2>&1 || echo "[node exited with status $?]"
        else
          MODE=$mode SIZE_MB=$size bun run probe.ts 2>&1 || echo "[bun exited with status $?]"
        fi
      done
    done
  done
done
