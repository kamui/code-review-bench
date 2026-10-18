#!/usr/bin/env bash
# Usage: run.sh <probe file> <hono checkout> [node|bun]   (environment variables are passed to the probe)
# Copies the probe into <checkout>/_probe/, then runs it under bun, or bundles it with `bun build --target=node`
# and runs the bundle under node. NODE_BIN pins the node binary.
set -euo pipefail
node_bin="${NODE_BIN:-node}"
probe="$1"; checkout="$2"; runtime="${3:-node}"
name="$(basename "$probe" .ts)"
mkdir -p "$checkout/_probe"
cp "$probe" "$checkout/_probe/$name.ts"
cd "$checkout/_probe"
if [ "$runtime" = bun ]; then
  bun run "$name.ts" 2>&1 || echo "[bun exited with status $?]"
else
  if [ ! -f "$name.node.mjs" ] || [ "$name.ts" -nt "$name.node.mjs" ] || [ -n "${REBUILD:-}" ]; then
    bun build "$name.ts" --target=node --outfile="$name.node.mjs" >/dev/null
  fi
  "$node_bin" --expose-gc "$name.node.mjs" 2>&1 || echo "[node exited with status $?]"
fi
