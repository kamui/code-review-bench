#!/bin/bash
# usage: run-chain.sh <clone dir> <label> <result file> [patched schedule_one.go]
# Lays the probe (and, if given, a patched schedule_one.go) over the clone with `go test -overlay`; the clone is not changed.
set -uo pipefail
. <scratch>/scripts/goenv.sh
clone=$1; label=$2; result=$3; patched=${4:-}
probe=$S/probe-src/chain_probe_test.go
ov=$S/out/overlay-chain-$label.json
if [ -n "$patched" ]; then
  printf '{"Replace": {"%s/pkg/scheduler/zz_p1_chain_probe_test.go": "%s", "%s/pkg/scheduler/schedule_one.go": "%s"}}\n' "$clone" "$probe" "$clone" "$patched" > $ov
else
  printf '{"Replace": {"%s/pkg/scheduler/zz_p1_chain_probe_test.go": "%s"}}\n' "$clone" "$probe" > $ov
fi
cd "$clone"
{
  echo "# label: $label"
  echo "# commit: $(git rev-parse HEAD)"
  echo "# git status --short (the clone is unchanged): [$(git status --short)]"
  if [ -n "$patched" ]; then echo "# schedule_one.go replaced by the patched copy; diff against the commit's file:"; diff -u pkg/scheduler/schedule_one.go "$patched" | sed -E '1,2s/\t.*$//' | sed -E "1s|.*|--- a/pkg/scheduler/schedule_one.go|; 2s|.*|+++ b/pkg/scheduler/schedule_one.go|"; fi
  echo "# command: go test -v -p 4 -vet=off -count=1 -overlay <overlay> -run '^TestP1Chain\$' ./pkg/scheduler"
  go test -v -p 4 -vet=off -count=1 -overlay $ov -run '^TestP1Chain$' ./pkg/scheduler 2>&1 | grep -Ev '^ +[a-z_]+\.go:[0-9]+: [IEW][0-9]{4} '
  echo "# exit: ${PIPESTATUS[0]}"
  echo "# the scheduler's own log lines (indented file.go:line: I/E/W...) are dropped from this output"
} > "$result"
