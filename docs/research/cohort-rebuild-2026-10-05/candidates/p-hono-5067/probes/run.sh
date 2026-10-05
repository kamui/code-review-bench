#!/usr/bin/env bash
# Usage: run.sh <group> <label> <hono checkout> [extra node flags]
# Copies probes/<group>/probe.ts into <checkout>/_probe/, runs it under bun, then bundles it
# for node with `bun build` and runs the bundle under node. Prints both outputs.
# NODE_BIN pins the node binary so every checkout runs on the same node version.
set -euo pipefail
node_bin="${NODE_BIN:-node}"
here="$(cd "$(dirname "$0")" && pwd)"
group="$1"; label="$2"; checkout="$3"; shift 3
mkdir -p "$checkout/_probe"
cp "$here/$group/probe.ts" "$checkout/_probe/probe.ts"
# PROBE_DEPS: a node_modules directory holding third-party packages the probe imports (H2 uses @hono/node-server).
if [ -n "${PROBE_DEPS:-}" ]; then ln -sfn "$PROBE_DEPS" "$checkout/_probe/node_modules"; fi
echo "##### $group at $label: $(git -C "$checkout" rev-parse HEAD 2>/dev/null || echo "$label (tarball)")"
echo
echo "##### runtime: bun $(bun --version)"
(cd "$checkout/_probe" && bun run probe.ts) 2>&1 || echo "[bun exited with status $?]"
echo
echo "##### runtime: node $("$node_bin" --version) (bundle built by bun build --target=node)"
(cd "$checkout/_probe" && bun build probe.ts --target=node --outfile=probe.node.mjs >/dev/null)
(cd "$checkout/_probe" && "$node_bin" "$@" probe.node.mjs) 2>&1 || echo "[node exited with status $?]"
