#!/bin/sh
# Usage: run-all.sh   (from the scratch directory; see environment.txt for the layout)
# Rebuilds every result file of this dossier: each group's probes against the base, the head,
# the patched variants of the head and three later releases, with TypeScript 5.1.3, then the
# base and the head again with TypeScript 5.2.2, the run-time probes and the growth table.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
cd "$here"
rm -rf results
later="v10.43.0=trees/v10.43.0 v10.43.4=trees/v10.43.4 v10.43.6=trees/v10.43.6 v10.45.2=trees/v10.45.2"
variants() { for v in "$@"; do printf '%s=variants/%s/%s ' "$v" "$group" "$v"; done; }

group=M1
m1="fix-union-pairs fix-same-input nondistributive fix-optional fix-brand fix-array fix-intersection both-union-pairs-and-intersection both-union-pairs-and-array both-union-pairs-and-brand all-four-narrow-fixes"
tools/run-pair.sh M1 base=trees/base head=trees/head $(variants $m1) $later
tools/run-pair.sh M1/known base=trees/base head=trees/head $(variants $m1) $later

group=M2
m2="revert-second-gate fix-keep-object fix-nullish later-fix-shape fix-missing-first both-keep-object-and-missing-first both-nullish-and-missing-first fix-same-input fix-union-pairs"
tools/run-pair.sh M2 base=trees/base head=trees/head $(variants $m2) $later
tools/run-pair.sh M2/known base=trees/base head=trees/head $(variants $m2) $later

group=M3
m3="revert-first-gate fix-any site-middleware-ctx site-resolve-options site-procedure-builder-ctx sites-builder-and-resolver"
tools/run-pair.sh M3 base=trees/base head=trees/head $(variants $m3) $later

tools/run-pair.sh C1 base=trees/base head=trees/head $later
tools/run-pair.sh C1n base=trees/base head=trees/head $later

for g in M1 M2 M3 C1 C1n; do
  PROBE_TS="$here/env-ts522/node_modules/typescript" SUFFIX=-ts522 tools/run-pair.sh $g base=trees/base head=trees/head
done
PROBE_EXACT_OPTIONAL=1 SUFFIX=-exact-optional tools/run-pair.sh C1n base=trees/base head=trees/head

for t in base head; do
  bun probes/M1/runtime/runtime.ts trees/$t > results/M1/runtime-$t.txt
  bun probes/M2/runtime/runtime.ts trees/$t > results/M2/runtime-$t.txt
done

mkdir -p results/M1/growth
for t in base head; do
  node tools/count.cjs trees/$t probes/M1/growth/variants-*.ts | sed -e "s#$here#<scratch>#g" > results/M1/growth/result-$t.txt
  PROBE_TS="$here/env-ts522/node_modules/typescript" node tools/count.cjs trees/$t probes/M1/growth/variants-2-middlewares-4.ts probes/M1/growth/variants-5-middlewares-3.ts probes/M1/growth/variants-12-middlewares-2.ts probes/M1/growth/variants-15-middlewares-2.ts | sed -e "s#$here#<scratch>#g" > results/M1/growth/result-$t-ts522.txt
done
echo finished

# Cross-checks added after the first pass: one wider pairing fix against both groups, each
# group's narrow fix against the other group, the saved optional-key fix against C1n, and the
# run-time probe of M3.
tools/run-pair.sh M1 fix-fitting-pairs=variants/M1/fix-fitting-pairs fix-missing-first=variants/M1/fix-missing-first
tools/run-pair.sh M1/known fix-fitting-pairs=variants/M1/fix-fitting-pairs
tools/run-pair.sh M2 fix-fitting-pairs=variants/M2/fix-fitting-pairs
tools/run-pair.sh M2/known fix-fitting-pairs=variants/M2/fix-fitting-pairs
tools/run-pair.sh M3 fix-fitting-pairs=variants/M1/fix-fitting-pairs
tools/run-pair.sh C1n fix-optional=variants/M1/fix-optional
tools/run-pair.sh C1 fix-optional=variants/M1/fix-optional
python3 tools/table.py results/M1 head fix-union-pairs fix-missing-first fix-fitting-pairs > results/M1/table-cross.md
python3 tools/table.py results/M1/known head fix-fitting-pairs > results/M1/known/table-cross.md
python3 tools/table.py results/M2 head fix-missing-first fix-union-pairs fix-fitting-pairs > results/M2/table-cross.md
python3 tools/table.py results/M2/known head fix-fitting-pairs > results/M2/known/table-cross.md
for t in base head; do bun probes/M3/runtime/runtime.ts trees/$t > results/M3/runtime-$t.txt; done
# The main tables last: table.py also writes table.json beside each table.md.
python3 tools/table.py results/M1 base head $m1 v10.43.0 v10.43.4 v10.43.6 v10.45.2 > results/M1/table.md
python3 tools/table.py results/M1/known base head $m1 v10.43.0 v10.43.4 v10.43.6 v10.45.2 > results/M1/known/table.md
python3 tools/table.py results/M2 base head $m2 v10.43.0 v10.43.4 v10.43.6 v10.45.2 > results/M2/table.md
python3 tools/table.py results/M2/known base head $m2 v10.43.0 v10.43.4 v10.43.6 v10.45.2 > results/M2/known/table.md
python3 tools/table.py results/M3 base head $m3 v10.43.0 v10.43.4 v10.43.6 v10.45.2 > results/M3/table.md
python3 tools/table.py results/C1 base head v10.43.0 v10.43.4 v10.43.6 v10.45.2 > results/C1/table.md
python3 tools/table.py results/C1n base head v10.43.0 v10.43.4 v10.43.6 v10.45.2 > results/C1n/table.md
echo finished-cross

# Two probes without their expected-error marks, so that the compiler's refusal text is saved.
tools/run-pair.sh M1/refusal-text base=trees/base head=trees/head
tools/run-pair.sh M2/refusal-text base=trees/base head=trees/head

# Two limits of fix-same-input.
tools/run-pair.sh M1/extra base=trees/base head=trees/head fix-same-input=variants/M1/fix-same-input fix-union-pairs=variants/M1/fix-union-pairs fix-optional=variants/M1/fix-optional
