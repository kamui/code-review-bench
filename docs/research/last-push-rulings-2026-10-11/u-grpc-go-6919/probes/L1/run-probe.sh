#!/usr/bin/env bash
# Run the L1 probe in one checkout of grpc-go.
#   run-probe.sh <scratch> <checkout-dir> <label>
# <scratch> holds the Go module and build caches. <checkout-dir> is a git worktree of grpc-go at the commit under
# test. The probe file is copied in, run once per interval with the project's own test harness, and removed.
set -uo pipefail
S=$1 TREE=$2 LABEL=$3
HERE=$(cd "$(dirname "$0")" && pwd)
export GOTOOLCHAIN=go1.21.6 GOMODCACHE="$S/gomodcache" GOCACHE="$S/gocache" GOPATH="$S/gopath" GOFLAGS=-mod=readonly
PKG=xds/internal/xdsclient/transport
cp "$HERE/zz_l1_probe_test.go" "$TREE/$PKG/zz_l1_probe_test.go"
trap 'rm -f "${TREE:?}/${PKG:?}/zz_l1_probe_test.go"' EXIT
cd "$TREE"
echo "# $LABEL: $(git rev-parse HEAD) $(git status --porcelain -- $PKG/loadreport.go | tr -s ' ')"
run() {
  local name=$1 seconds=$2 nanos=$3
  echo
  echo "## $name (seconds=$seconds nanos=$nanos)"
  PROBE_SECONDS=$seconds PROBE_NANOS=$nanos PROBE_WINDOW_MS=3000 \
    go test -count=1 -v -run 'Test/ProbeL1$' ./$PKG/ 2>&1 |
    grep -E 'PROBE|WARNING loadreport|ERROR|panic:|NewTicker|loadreport\.go:[0-9]+ \+|^(ok|FAIL|--- )' |
    sed -E 's/^ *tlogger\.go:[0-9]+: //; s/ *\(t=\+[^)]*\)//; s/127\.0\.0\.1:[0-9]+/127.0.0.1:<port>/g; s/ \+0x[0-9a-f]+$//; s/\(0x[0-9a-f, x]*\)/(…)/; s/[0-9]+\.[0-9]+s\)?$/<time>/; s#/[^ ]*/(base|head|fix[^/]*)/#<checkout>/#'
  echo "exit status of go test: ${PIPESTATUS[0]}"
}
run "A. ordinary interval, 50 ms" 0 50000000
run "B. largest whole-second interval Go can hold, 9223372036 s (292.3 years)" 9223372036 0
run "C. just past Go's range, 9223372037 s (292.3 years)" 9223372037 0
run "D. inside protobuf's range and outside Go's, 10000000000 s (317 years)" 10000000000 0
run "E. protobuf's largest duration, 315576000000 s (10,000 years)" 315576000000 0
run "F. outside protobuf's range, 315576000001 s" 315576000001 0
run "G. negative, inside protobuf's range and outside Go's, -10000000000 s" -10000000000 0
run "H. negative and small, -1 s" -1 0
run "I. zero" 0 0
