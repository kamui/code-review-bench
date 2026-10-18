#!/bin/bash
# Rebuilds every result file of this dossier from an empty results directory:
# each group's probes against base, head and the patched variants with TypeScript 5.1.3, base and head again
# with TypeScript 5.2.2, three later releases, the run-time probe of M1, and the tables.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd); cd "$here"
rm -rf results
bash tools/run-m1.sh
bash tools/run-m2.sh
bash tools/run-m3.sh
bash tools/run-c1.sh
bash tools/run-later.sh
for t in base head; do bun runtime/m1-runtime.ts trees/$t > results/M1/runtime-$t.txt; done
python3 tools/table.py results/M1 base head pe-j1-copy-intersection m1-builtin-check both-copy-intersection-and-builtin-check gt-j7-array-check both-array-check-and-builtin-check fix-same-input > results/M1/table.md
python3 tools/table.py results/M1-known base head pe-j1-copy-intersection m1-builtin-check both-copy-intersection-and-builtin-check gt-j7-array-check both-array-check-and-builtin-check fix-same-input > results/M1-known/table.md
python3 tools/table.py results/M2 base head m2-never-check gt-j1-revert-second-gate both-revert-second-gate-and-never-check > results/M2/table.md
python3 tools/table.py results/M2-known base head m2-never-check gt-j1-revert-second-gate both-revert-second-gate-and-never-check > results/M2-known/table.md
python3 tools/table.py results/M3 base head gt-j7-array-check > results/M3/table.md
python3 tools/table.py results/M3-known base head gt-j7-array-check > results/M3-known/table.md
python3 tools/table.py results/C1 base head gt-j2-fix-any gt-j2-revert-first-gate gt-j1-revert-second-gate > results/C1/table.md
python3 tools/table.py results/C1-known base head gt-j2-fix-any gt-j2-revert-first-gate gt-j1-revert-second-gate > results/C1-known/table.md
for g in M1 M2 M3 C1; do python3 tools/table.py results/$g base head v10.43.4 v10.43.6 v10.45.2 > results/$g/table-releases.md; done
echo finished
