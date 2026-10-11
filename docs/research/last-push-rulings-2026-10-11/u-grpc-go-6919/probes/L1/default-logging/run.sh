#!/usr/bin/env bash
# run.sh <scratch> <checkout-dir> <label>: run main.go in one checkout with grpc-go's standard logger at three settings.
set -uo pipefail
S=$1 TREE=$2 LABEL=$3
HERE=$(cd "$(dirname "$0")" && pwd)
export GOTOOLCHAIN=go1.21.6 GOMODCACHE="$S/gomodcache" GOCACHE="$S/gocache" GOPATH="$S/gopath" GOFLAGS=-mod=readonly
mkdir -p "$TREE/xds/zzl1probe" && cp "$HERE/main.go" "$TREE/xds/zzl1probe/main.go"
trap 'rm -f "${TREE:?}/xds/zzl1probe/main.go"; rmdir "${TREE:?}/xds/zzl1probe"' EXIT
cd "$TREE" && go build -o "$S/zzl1probe-$LABEL" ./xds/zzl1probe || exit 1
echo "# $LABEL: $(git rev-parse HEAD); the server sends load_reporting_interval = 10000000000 s"
for level in "" warning info; do
  echo
  echo "## GRPC_GO_LOG_SEVERITY_LEVEL=${level:-<unset, the default>}"
  out=$(env -u GRPC_GO_LOG_VERBOSITY_LEVEL GRPC_GO_LOG_SEVERITY_LEVEL="$level" PROBE_SECONDS=10000000000 "$S/zzl1probe-$LABEL" 2>&1)
  echo "lines written by the program: $(printf '%s\n' "$out" | grep -vc '^PROBE')"
  printf '%s\n' "$out" | grep -E 'load_reporting_interval|LRS' | sed -E 's/^[0-9/]+ [0-9:]+ //; s/127\.0\.0\.1:[0-9]+/127.0.0.1:<port>/g' | sort | uniq -c
done
