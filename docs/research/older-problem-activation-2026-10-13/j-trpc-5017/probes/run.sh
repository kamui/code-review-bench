#!/bin/sh
# usage: run.sh <scratch> <repository>
# Builds base, head and eight patched variants of tRPC's server source in <scratch>, outside the repository, and
# type-checks the saved optional-field probe and four saved array probes with their controls against each.
# <scratch>/env must hold typescript 5.1.3, zod 3.20.2 and @types/node 18.16.16 (bun add, in <scratch>/env).
set -eu
S=$1; R=$2
M=$HOME/.t3/bench-cache/mirrors/j-trpc-5017.git
HERE=$R/docs/research/older-problem-activation-2026-10-13/j-trpc-5017/probes
N2=$R/docs/research/cohort-rebuild-2026-10-05/second-pass/candidates/j-trpc-5017/probes/N2
K2=$R/docs/research/last-push-rulings-2026-10-11/kinds/prepared/K2/probes
U=packages/server/src/core/internals/utils.ts
ARRAY="04-uses-tuple-spread.ts 05-control-tuple-spread-no-middleware.ts 07-uses-class-private.ts 08-control-class-private-no-middleware.ts 13-promise-all.ts 14-control-promise-all-no-middleware.ts 15-object-values.ts 16-control-object-values-no-middleware.ts"
mkdir -p "$S/trees/base" "$S/trees/head" "$S/variants" "$S/probes" "$S/results"
git --git-dir="$M" archive 2abb2d5cd19740be37272dac6ad7fdd36244ae54 packages/server/src packages/client/src | tar -x -C "$S/trees/base"
git --git-dir="$M" archive 7dc04a7e94654dfad6ef1289dfe01a0a206fff3b packages/server/src packages/client/src | tar -x -C "$S/trees/head"
for pair in base+copy-only base+checks-only base+array-check base+copy-intersection head+copy-only head+checks-only head+array-check head+copy-intersection; do
  tree=${pair%%+*}; patch=${pair#*+}
  rm -rf "$S/variants/$pair"; cp -r "$S/trees/$tree" "$S/variants/$pair"
  (cd "$S/variants/$pair" && patch -p1 --quiet --no-backup-if-mismatch < "$HERE/patches/$patch-at-$tree.patch")
done
cp "$K2/check.cjs" "$S/probes/k2-check.cjs"
for p in $ARRAY; do cp "$K2/$p" "$S/probes/"; done
ln -sfn "$S/env/node_modules" "$S/probes/node_modules"
for name in base head base+copy-only head+copy-only base+checks-only head+checks-only base+array-check head+array-check base+copy-intersection head+copy-intersection; do
  case $name in base|head) tree=$S/trees/$name ;; *) tree=$S/variants/$name ;; esac
  ln -sfn "$S/env/node_modules" "$tree/node_modules"
  mkdir -p "$S/runs/$name"
  ln -sfn "$tree" "$S/runs/$name/clone"; ln -sfn "$S/env/node_modules" "$S/runs/$name/node_modules"
  cp "$N2/probe.ts" "$S/runs/$name/N2-probe.ts"; cp "$N2/check.cjs" "$S/runs/$name/N2-check.cjs"
  { echo "variant $name"; echo "COMMAND node N2-check.cjs N2-probe.ts"; (set +e; cd "$S/runs/$name" && node N2-check.cjs N2-probe.ts; echo "EXIT $?"); } > "$S/results/optional-field-$name.txt" 2>&1
  { echo "variant $name"; echo "COMMAND node k2-check.cjs <tree> 04 05 07 08 13 14 15 16"; (set +e; cd "$S/probes" && node k2-check.cjs "$tree" $ARRAY; echo "EXIT $?"); } > "$S/results/array-input-$name.txt" 2>&1
done
