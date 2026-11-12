#!/usr/bin/env bash
# Usage: run.sh <scratch> <tree>   where <scratch>/<tree> is a checkout of honojs/hono:
#   base           9728702911073aec5a63a3ba2840b7240e5d3205
#   head           5226d4165d48643586152614cbd07422a0ab7a22
#   head-fix-gtp5  head + fix-GT-p5.patch (fix-M6 of rule-v4-candidates ruling 15)
#   head-fix-json  head + fix-json.patch
#   head-fix-both  head + fix-json+GT-p5.patch
#   head-fix-gtp4  head + fix-GT-p4.patch (fix-M4a+M4b of owner-rulings ruling 1)
#   base-fix-json  base + fix-json-base.patch
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
scratch="$1"
tree="$2"
mkdir -p "$scratch/out"
cp "$here/probe.ts" "$scratch/$tree/probe-n1.ts"
bun build "$scratch/$tree/probe-n1.ts" --target=node --format=esm --outfile "$scratch/out/n1-$tree.mjs" >/dev/null
echo "##### $tree ($(git -C "$scratch/$tree" rev-parse HEAD)$(git -C "$scratch/$tree" diff --quiet -- src || echo ' + local patch'))"
git -C "$scratch/$tree" diff -- src | grep -E '^[+-]const' || true
echo "### node $(node --version)"
node "$scratch/out/n1-$tree.mjs"
echo "### bun $(bun --version)"
bun "$scratch/out/n1-$tree.mjs"
echo "### deno $(deno --version | head -1)"
deno run --quiet "$scratch/out/n1-$tree.mjs"
