#!/bin/bash
# Runs M2's probes and GT-j1's saved failing probes against base, head and M2's patched variants.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd); cd "$here"
args=(base=trees/base head=trees/head)
for v in m2-never-check gt-j1-revert-second-gate both-revert-second-gate-and-never-check; do args+=("$v=variants/M2/$v"); done
sh tools/run-pair.sh M2 "${args[@]}"
sh tools/run-pair.sh M2-known "${args[@]}"
PROBE_TS="$here/env-ts522/node_modules/typescript" SUFFIX=-ts522 sh tools/run-pair.sh M2 base=trees/base head=trees/head
