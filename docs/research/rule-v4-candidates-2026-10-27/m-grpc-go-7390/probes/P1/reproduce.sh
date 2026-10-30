#!/bin/sh
set -eu
# Execute from a scratch clone at either pinned commit. Evidence is written to a caller-supplied allowed path.
probe_dir=/home/jack/.cache/regrade-v4/docs/research/rule-v4-candidates-2026-10-27/m-grpc-go-7390/probes/P1
export GOMODCACHE=/home/jack/.cache/rule-v4-candidates-scratch/m-grpc-go-7390/m-grpc-go-7390/gomodcache
export GOCACHE=/home/jack/.cache/rule-v4-candidates-scratch/m-grpc-go-7390/gocache
export GOTMPDIR=/home/jack/.cache/rule-v4-candidates-scratch/m-grpc-go-7390
cp "$probe_dir/probe_test.go" p1_probe_test.go
python3 "$probe_dir/instrument.py"
go test -race -run '^TestP1$' -v -count=10 .
