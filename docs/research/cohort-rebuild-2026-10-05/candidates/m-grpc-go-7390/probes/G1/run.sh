#!/usr/bin/env bash
# Usage: run.sh <grpc-go checkout> <scratch dir for Go caches>
# Copies the probe into the checkout, runs every configuration and prints the results.
set -u
src=$1
scratch=$2
here=$(cd "$(dirname "$0")" && pwd)
export GOPATH=$scratch/gopath GOMODCACHE=$scratch/gopath/pkg/mod GOCACHE=$scratch/gocache GOFLAGS=-mod=mod GOTOOLCHAIN=local
cp "$here/g1_probe_test.go" "$src/g1_probe_test.go"
cd "$src" || exit 1
echo "commit: $(git rev-parse HEAD)"
run() {
  echo
  echo "=== $*"
  env "$@" 2>&1 | grep -v '^=== RUN'
}
run go test -count=1 -v -run 'TestG1Natural$' .
run go test -count=1 -v -run 'TestG1Natural$' .
run go test -count=1 -v -race -run 'TestG1Natural$' .
run GOMAXPROCS=4 G1_SPINNERS=16 G1_ROUNDS=300 go test -count=1 -v -run 'TestG1Natural$' .
run GOMAXPROCS=2 G1_SPINNERS=8 G1_ROUNDS=200 go test -count=1 -v -timeout 20m -run 'TestG1Natural$' .
run GOMAXPROCS=1 G1_SPINNERS=4 G1_ROUNDS=200 go test -count=1 -v -timeout 20m -run 'TestG1Natural$' .
run go test -count=1 -v -run 'TestG1Widened$' .
run go test -count=1 -v -race -run 'TestG1Widened$' .
run go test -count=1 -v -race -run 'TestG1UnderRPCLoad$' .
run go test -count=1 -v -run 'TestG1UnderRPCLoad$' .
run go test -count=1 -v -run 'TestG1LockAfterReturn$' .
run GOMAXPROCS=1 G1_ROUNDS=50 go test -count=1 -v -run 'TestG1LockAfterReturn$' .
run GOMAXPROCS=1 G1_SPINNERS=4 G1_ROUNDS=20 go test -count=1 -v -timeout 20m -run 'TestG1LockAfterReturn$' .
rm -f "$src/g1_probe_test.go"
