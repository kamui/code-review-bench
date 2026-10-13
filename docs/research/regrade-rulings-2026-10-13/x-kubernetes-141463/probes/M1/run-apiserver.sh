#!/bin/bash
# usage: run-apiserver.sh <base|head> <probe file> <result file>
set -uo pipefail
S=$S
. $S/scripts/goenv.sh
export PATH=$S/etcd:$PATH
side=$1; probe=$2; result=$3
clone=$S/$side
printf '{"Replace": {"%s/test/integration/scheduler/queueing/zz_m1_apiserver_probe_test.go": "%s"}}\n' "$clone" "$probe" > $S/out/overlay-apiserver-$side.json
cd "$clone"
{
  echo "# commit: $(git rev-parse HEAD)"
  echo "# git status --short (the clone is unchanged): [$(git status --short)]"
  echo "# etcd: $(etcd --version | head -1)"
  echo "# command: go test -v -p 4 -vet=off -count=1 -timeout 15m -overlay <overlay> -run '^TestM1APIServer\$' ./test/integration/scheduler/queueing"
  go test -v -p 4 -vet=off -count=1 -timeout 15m -overlay $S/out/overlay-apiserver-$side.json -run '^TestM1APIServer$' ./test/integration/scheduler/queueing 2>&1
  echo "# exit: $?"
} > "$result"
