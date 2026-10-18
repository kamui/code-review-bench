#!/bin/bash
# usage: run-apiserver.sh <clone dir> <label> <raw result file> [patched schedule_one.go]
set -uo pipefail
. <scratch>/scripts/goenv.sh
export PATH=$S/etcd:$PATH
clone=$1; label=$2; result=$3; patched=${4:-}
probe=$S/probe-src/apiserver_probe_test.go
ov=$S/out/overlay-apiserver-$label.json
if [ -n "$patched" ]; then
  printf '{"Replace": {"%s/test/integration/scheduler/queueing/zz_p1_apiserver_probe_test.go": "%s", "%s/pkg/scheduler/schedule_one.go": "%s"}}\n' "$clone" "$probe" "$clone" "$patched" > $ov
else
  printf '{"Replace": {"%s/test/integration/scheduler/queueing/zz_p1_apiserver_probe_test.go": "%s"}}\n' "$clone" "$probe" > $ov
fi
cd "$clone"
{
  echo "# label: $label"
  echo "# commit: $(git rev-parse HEAD)"
  echo "# git status --short (the clone is unchanged): [$(git status --short)]"
  if [ -n "$patched" ]; then echo "# schedule_one.go replaced by the patched copy: $(basename $patched)"; fi
  echo "# etcd: $(etcd --version | head -1)"
  echo "# command: go test -v -p 4 -vet=off -count=1 -timeout 20m -overlay <overlay> -run '^TestP1(APIServer|ManyNodes)\$' ./test/integration/scheduler/queueing"
  go test -v -p 4 -vet=off -count=1 -timeout 20m -overlay $ov -run '^TestP1(APIServer|ManyNodes)$' ./test/integration/scheduler/queueing 2>&1
  echo "# exit: $?"
} > "$result"
