#!/usr/bin/env bash
# Usage: capped.sh <label> <hono checkout>   (run measure.sh first; it builds probe.node.mjs)
# Runs the same 100 MB upload under node inside a control group limited to 420 MB of memory with no swap,
# three times. A process that needs more than the limit is killed by the kernel.
# NODE_BIN pins the node binary so every checkout runs on the same node version.
set -uo pipefail
node_bin="${NODE_BIN:-node}"
label="$1"; checkout="$2"
cd "$checkout/_probe"
echo "##### H3 memory cap at $label: node $("$node_bin" --version), MemoryMax=420M, MemorySwapMax=0, upload=100MB, c.req.parseBody()"
for _ in 1 2 3; do
  systemd-run --user --scope -q -p MemoryMax=420M -p MemorySwapMax=0 \
    env MODE=parseBody SIZE_MB=100 "$node_bin" --expose-gc probe.node.mjs 2>&1 | cut -c1-260
  echo "exit status: ${PIPESTATUS[0]}"
done
