#!/usr/bin/env bash
# Usage: run.sh <scratch>   where <scratch> holds worktrees of the Hono mirror:
#   base          9728702911073aec5a63a3ba2840b7240e5d3205
#   head          5226d4165d48643586152614cbd07422a0ab7a22
#   head-fixm4b   head with fix-M4b.patch applied (patch copied from
#                 docs/research/rule-v4-candidates-2026-10-27/p-hono-5067/probes/M4b/)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
scratch="$1"
mkdir -p "$scratch/out"
for tree in base head head-fixm4b; do
  cp "$here/probe.ts" "$scratch/$tree/probe-w3.ts"
  bun build "$scratch/$tree/probe-w3.ts" --target=node --format=esm --outfile "$scratch/out/headers-$tree.mjs" >/dev/null
  echo "##### $tree ($(git -C "$scratch/$tree" rev-parse HEAD)$(git -C "$scratch/$tree" diff --quiet -- src || echo ' + local patch'))"
  echo "### node $(node --version)"
  node "$scratch/out/headers-$tree.mjs"
  echo "### bun $(bun --version)"
  bun "$scratch/out/headers-$tree.mjs"
  echo "### deno $(deno --version | head -1)"
  deno run --quiet "$scratch/out/headers-$tree.mjs"
done
