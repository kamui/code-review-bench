#!/bin/bash
# usage: run-chain.sh <base|head> <probe file> <result file>
set -euo pipefail
S=$S
. $S/scripts/goenv.sh
side=$1; probe=$2; result=$3
clone=$S/$side
printf '{"Replace": {"%s/pkg/scheduler/zz_m1_chain_probe_test.go": "%s"}}\n' "$clone" "$probe" > $S/out/overlay-chain-$side.json
cd "$clone"
{
  echo "# commit: $(git rev-parse HEAD)"
  echo "# git status --short (the clone is unchanged): [$(git status --short)]"
  echo "# command: go test -v -p 4 -vet=off -count=1 -overlay <overlay> -run '^TestM1Chain\$' ./pkg/scheduler"
  go test -v -p 4 -vet=off -count=1 -overlay $S/out/overlay-chain-$side.json -run '^TestM1Chain$' ./pkg/scheduler 2>&1
} > "$result"
