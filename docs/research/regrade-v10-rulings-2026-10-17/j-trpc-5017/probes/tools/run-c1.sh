#!/bin/bash
# Runs C1's probes and the saved failing probes of GT-j2 and GT-j1 against base, head and the known problems' fixes.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd); cd "$here"
args=(base=trees/base head=trees/head)
for v in gt-j2-fix-any gt-j2-revert-first-gate gt-j1-revert-second-gate; do args+=("$v=variants/C1/$v"); done
sh tools/run-pair.sh C1 "${args[@]}"
sh tools/run-pair.sh C1-known "${args[@]}"
PROBE_TS="$here/env-ts522/node_modules/typescript" SUFFIX=-ts522 sh tools/run-pair.sh C1 base=trees/base head=trees/head
