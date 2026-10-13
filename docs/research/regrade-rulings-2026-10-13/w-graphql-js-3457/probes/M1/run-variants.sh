#!/usr/bin/env bash
# Builds each fix variant and runs every case against it, one process at a time.
#   run-variants.sh <scratch directory holding clone-base, clone-head and clone-fix-*> <results directory>
set -euo pipefail
scratch=$1
out=$2
here=$(cd "$(dirname "$0")" && pwd)
variants="fastpath with-arguments-only both maintainers-pr-3967 memo"

for v in $variants; do
  (cd "$scratch/clone-fix-$v" && node resources/build-npm.js > "$scratch/build-fix-$v.log" 2>&1)
done

# The rule's own test file and the whole validation suite, at base, head and each variant.
for c in base head $(for v in $variants; do echo "fix-$v"; done); do
  (cd "$scratch/clone-$c" && ./node_modules/.bin/mocha 'src/validation/__tests__/**/*-test.ts' 2>&1 \
    | grep -E 'passing|failing|pending' | tr '\n' ' ' | sed "s/^/$c validation suite: /"; echo) >> "$out/project-tests.txt"
done

for round in 1 2; do
  for c in base head; do
    node "$here/timing.cjs" dist "$scratch/clone-$c" "$c" "$out/timing-dist-$c-variants$round.json" \
      --sizes 300,1000 --reps 5 > /dev/null
  done
  for v in $variants; do
    node "$here/timing.cjs" dist "$scratch/clone-fix-$v" "fix-$v" "$out/timing-dist-fix-$v-fix$round.json" \
      --sizes 300,1000 --reps 5 > /dev/null
  done
done

for c in base head $(for v in $variants; do echo "fix-$v"; done); do
  node "$here/w1.cjs" "$scratch/clone-$c" "$c" >> "$out/gt-w1-case.txt"
  node "$here/count.cjs" "$scratch/clone-$c" "$c" "$out/count-$c.json" > /dev/null
done
echo done
