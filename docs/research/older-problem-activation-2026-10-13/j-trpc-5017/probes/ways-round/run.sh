#!/bin/sh
# usage: run.sh <scratch> <repository>
# Type-checks and executes the way-round probes of this directory against tRPC's server source at the base and at
# the head of pull request 5017. <scratch> is outside the repository and must hold what ../run.sh builds there:
# <scratch>/env with typescript 5.1.3, zod 3.20.2 and @types/node 18.16.16, and <scratch>/trees/base and
# <scratch>/trees/head with a node_modules link to <scratch>/env/node_modules.
set -eu
S=$1; R=$2
HERE=$R/docs/research/older-problem-activation-2026-10-13/j-trpc-5017/probes/ways-round
K2=$R/docs/research/last-push-rulings-2026-10-11/kinds/prepared/K2/probes
mkdir -p "$S/ways/probes" "$S/results"
cp "$HERE"/*.ts "$S/ways/probes/"; rm "$S/ways/probes/run-use.ts"
cp "$K2/check.cjs" "$S/ways/probes/k2-check.cjs"
ln -sfn "$S/env/node_modules" "$S/ways/probes/node_modules"
for tree in base head; do
  { echo "COMMAND node k2-check.cjs <scratch>/trees/$tree *.ts"; (set +e; cd "$S/ways/probes" && node k2-check.cjs "$S/trees/$tree" *.ts; echo "EXIT $?"); } 2>&1 | sed "s#$S#<scratch>#g" > "$S/results/ways-round-typecheck-$tree.txt"
  mkdir -p "$S/ways/runs/$tree"
  cp "$S/ways/probes"/*.ts "$HERE/run-use.ts" "$S/ways/runs/$tree/"
  ln -sfn "$S/env/node_modules" "$S/ways/runs/$tree/node_modules"
  printf '{\n  "compilerOptions": {\n    "baseUrl": "../../../trees/%s",\n    "paths": { "@trpc/server": ["packages/server/src"], "@trpc/server/*": ["packages/server/src/*"] }\n  }\n}\n' "$tree" > "$S/ways/runs/$tree/tsconfig.json"
  { echo "COMMAND for each probe: bun run-use.ts ./<probe>   (in a folder whose tsconfig.json maps @trpc/server to <scratch>/trees/$tree/packages/server/src)"
    for p in $(cd "$S/ways/probes" && ls *.ts); do echo "## $p"; (cd "$S/ways/runs/$tree" && bun run-use.ts "./$p" 2>&1 | sed "s#$S#<scratch>#g" | cut -c1-500); done; } > "$S/results/ways-round-runtime-$tree.txt"
done
