#!/usr/bin/env bash
# Usage: run.sh <scratch>   where <scratch>/head is a checkout of the Hono mirror at
#   5226d4165d48643586152614cbd07422a0ab7a22. The script copies it to <scratch>/head-fixm4a-m4b and
#   applies fix-M4a+M4b.patch (copied unchanged from
#   docs/research/rule-v4-candidates-2026-10-27/p-hono-5067/probes/patches/). probe.ts is the round-2
#   probe (docs/research/audit-v2-r2-reconciliation-2026-11-04/probes/hono-media-type-headers/probe.ts), unchanged.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
scratch="$1"
tree="$scratch/head-fixm4a-m4b"
rm -rf "$tree"; cp -r "$scratch/head" "$tree"
git -C "$tree" apply "$here/fix-M4a+M4b.patch"
mkdir -p "$scratch/out"
cp "$here/probe.ts" "$tree/probe-w3.ts"
bun build "$tree/probe-w3.ts" --target=node --format=esm --outfile "$scratch/out/headers-fixm4a-m4b.mjs" >/dev/null
echo "##### head-fixm4a-m4b ($(git -C "$tree" rev-parse HEAD)$(git -C "$tree" diff --quiet -- src || echo ' + local patch'))"
echo "### node $(node --version)"
node "$scratch/out/headers-fixm4a-m4b.mjs"
echo "### bun $(bun --version)"
bun "$scratch/out/headers-fixm4a-m4b.mjs"
echo "### deno $(deno --version | head -1)"
deno run --quiet "$scratch/out/headers-fixm4a-m4b.mjs"
