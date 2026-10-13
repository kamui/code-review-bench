#!/bin/sh
# Runs the M1 probe. Usage: run.sh SCRATCH OUT
#
# SCRATCH holds four checkouts of grpc/grpc-go with m1_probe_test.go copied into each root:
#   base               daab56344e612097fd50c46c433de5d9b6013837
#   head               76ef33f44a600c3ed1a385979fd1dfbcade3fbb6
#   base-widened       base with patches/base-widened-window.patch
#   head-early-return  head with patches/head-return-after-idle.patch
# and the Go module cache of the benchmark's dependency archive under deps/m-grpc-go-7390.
set -u
S=$1
OUT=$2
export GOMODCACHE="$S/deps/m-grpc-go-7390/gomodcache" GOCACHE="$S/deps/m-grpc-go-7390/gocache"
export GOPROXY=off GOTOOLCHAIN=local GOFLAGS=-mod=mod

run() {
	name=$1
	dir=$2
	shift 2
	(cd "$S/$dir" && "$@") 2>&1 | grep -Ev '^=== (RUN|PAUSE|CONT|NAME)' >"$OUT/$name.txt"
}

# 1. Every case, 200 rounds each, race detector on, at both commits.
run race-base base env M1_ROUNDS=200 go test -p 4 -race -count=1 -timeout 90m -run 'TestM1' -v . &
run race-head head env M1_ROUNDS=200 go test -p 4 -race -count=1 -timeout 90m -run 'TestM1' -v . &
wait

# 2. The two main cases on two processors, no race detector, 200 rounds each.
run procs2-base base env GOMAXPROCS=2 M1_ROUNDS=200 go test -p 4 -count=1 -timeout 60m -run 'TestM1(Ready|Connecting)$' -v .
run procs2-head head env GOMAXPROCS=2 M1_ROUNDS=200 go test -p 4 -count=1 -timeout 60m -run 'TestM1(Ready|Connecting)$' -v .

# 3. The project's own test of an emptied SubConn, 50 times with the race detector.
run project-test-base base go test -p 4 -race -count=50 -timeout 60m -run 'Test/SubConnEmpty' -v ./test/
run project-test-head head go test -p 4 -race -count=50 -timeout 60m -run 'Test/SubConnEmpty' -v ./test/

# 4. One round with the library's own logging at verbosity 2.
for c in base head; do
	run verbose-$c $c env GRPC_GO_LOG_VERBOSITY_LEVEL=2 GRPC_GO_LOG_SEVERITY_LEVEL=info M1_ROUNDS=1 M1_WORKERS=1 \
		go test -p 4 -count=1 -run 'TestM1Ready/m1_passive/empty-slice' -v .
done

# 5. Base with the new attempt started 50 ms late, so that the lock stays free and the state IDLE for that long.
run base-widened base-widened env M1_ROUNDS=100 go test -p 4 -race -count=1 -timeout 60m -run 'TestM1(Ready|Connecting)$' -v .

# 6. Head with updateAddrs returning after it reports IDLE for an empty list.
run head-early-return head-early-return env M1_ROUNDS=100 go test -p 4 -race -count=1 -timeout 60m -run 'TestM1(Ready|Connecting|PickFirst)$' -v .
echo done >"$OUT/finished"
