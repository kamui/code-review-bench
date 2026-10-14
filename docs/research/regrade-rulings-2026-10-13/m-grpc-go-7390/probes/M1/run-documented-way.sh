#!/bin/sh
# Runs the ways the project itself handles "all addresses were removed". Usage: run-documented-way.sh SCRATCH OUT
# Needs m1_probe_test.go and m1_documented_way_test.go in the root of SCRATCH/base and SCRATCH/head.
set -u
S=$1
OUT=$2
export GOMODCACHE="$S/deps/m-grpc-go-7390/gomodcache" GOCACHE="$S/deps/m-grpc-go-7390/gocache"
export GOPROXY=off GOTOOLCHAIN=local GOFLAGS=-mod=mod
for c in base head; do
	(cd "$S/$c" && env M1_ROUNDS=50 go test -p 4 -race -count=1 -timeout 30m -run 'TestM1DocumentedWay' -v .) 2>&1 |
		grep -Ev '^=== (RUN|PAUSE|CONT|NAME)' >"$OUT/documented-way-$c.txt"
	(cd "$S/$c" && go test -p 4 -race -count=20 -timeout 30m \
		-run 'Test/(PickFirst_AddressesRemoved|PickFirst_ResolverError_ZeroAddresses_WithPreviousUpdate)$' -v ./test/) 2>&1 |
		grep -Ev '^=== (RUN|PAUSE|CONT|NAME)' | grep -Ev '^\s+(tlogger|pickfirst_test|stubserver|logging)\.go' >"$OUT/project-pickfirst-tests-$c.txt"
done
echo done >"$OUT/finished-documented-way"
