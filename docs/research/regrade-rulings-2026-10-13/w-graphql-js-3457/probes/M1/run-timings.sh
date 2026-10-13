#!/usr/bin/env bash
# Runs the base-and-head timing matrix, one process at a time.
#   run-timings.sh <scratch directory holding clone-base and clone-head> <results directory>
set -euo pipefail
scratch=$1
out=$2
here=$(cd "$(dirname "$0")" && pwd)
quiet() { grep -v -e 'rowserslist' -e 'update-db' -e 'do it regularly' || true; }

for round in 1 2 3; do
  for commit in base head; do
    node "$here/timing.cjs" dist "$scratch/clone-$commit" "$commit" "$out/timing-dist-$commit-round$round.json" \
      --sizes 100,200,300,1000 --reps 5 > /dev/null
  done
done
for commit in base head; do
  node "$here/timing.cjs" dist "$scratch/clone-$commit" "$commit" "$out/timing-dist-$commit-2000.json" \
    --sizes 2000 --reps 3 > /dev/null
done
for commit in base head; do
  node "$here/timing.cjs" dist "$scratch/clone-$commit" "$commit" "$out/timing-dist-$commit-allrules.json" \
    --sizes 300,1000 --reps 5 --rules all > /dev/null
  NODE_ENV=production node "$here/timing.cjs" dist "$scratch/clone-$commit" "$commit" "$out/timing-dist-$commit-production.json" \
    --sizes 300,1000 --reps 5 > /dev/null
done
for commit in base head; do
  node "$here/timing.cjs" src "$scratch/clone-$commit" "$commit" "$out/timing-src-$commit.json" \
    --sizes 100,200,300,1000 --reps 5 2>&1 | quiet > /dev/null
  node "$here/timing.cjs" src "$scratch/clone-$commit" "$commit" "$out/timing-src-$commit-noargs-2000.json" \
    --sizes 2000 --reps 3 --cases no-arguments 2>&1 | quiet > /dev/null
done
echo done
