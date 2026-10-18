#!/bin/bash
# Runs M1's probes and the known problems' failing probes against base, head and M1's patched variants.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd); cd "$here"
args=(base=trees/base head=trees/head)
for v in pe-j1-copy-intersection m1-builtin-check both-copy-intersection-and-builtin-check gt-j7-array-check both-array-check-and-builtin-check fix-same-input; do args+=("$v=variants/M1/$v"); done
sh tools/run-pair.sh M1 "${args[@]}"
sh tools/run-pair.sh M1-known "${args[@]}"
PROBE_TS="$here/env-ts522/node_modules/typescript" SUFFIX=-ts522 sh tools/run-pair.sh M1 base=trees/base head=trees/head
