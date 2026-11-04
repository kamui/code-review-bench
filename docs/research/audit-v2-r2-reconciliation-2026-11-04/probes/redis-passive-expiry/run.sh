#!/usr/bin/env bash
# Does a key set with EX 1 still exist 1.5 s later when Redis's active-expire cycle is off,
# or when the server is kept busy? And does the PR's own TTL test pass under those conditions?
#
# Usage: SCRATCH=<scratch> CLONE=<clone of s-seaweedfs-10735 at head 6c8fde6> REDIS_SERVER=<redis-server binary> ./run.sh
# The redis-server binary used: Debian trixie redis-tools 8.0.2, /usr/bin/redis-check-rdb, symlinked as
# <scratch>/bin/redis-server (the Redis binary picks its mode from its name).
# The test binary is built from the clone with the pinned module cache:
#   GOMODCACHE=<scratch>/gomodcache GOPROXY=off GOFLAGS=-mod=mod GOTOOLCHAIN=local \
#   go test -c -o <scratch>/redis2-head.test ./weed/filer/redis2/
set -u
PORT=6399
CLI="redis-cli -p $PORT"
mkdir -p "$SCRATCH/run"
"$REDIS_SERVER" --port $PORT --bind 127.0.0.1 --save '' --appendonly no --enable-debug-command local \
  --daemonize yes --pidfile "$SCRATCH/run/redis.pid" --dir "$SCRATCH/run" --logfile "$SCRATCH/run/redis.log"
for i in $(seq 50); do $CLI ping >/dev/null 2>&1 && break; sleep 0.1; done
echo "server: $($CLI info server | grep -E '^redis_version' | tr -d '\r')"

echo "== A. active expiry off: SET EX 1, sleep 1.5 s, then DBSIZE / EXISTS / DBSIZE"
$CLI debug set-active-expire 0
$CLI flushall >/dev/null
$CLI set k v EX 1
sleep 1.5
echo "DBSIZE before access (key still stored?): $($CLI dbsize)"
echo "EXISTS k: $($CLI exists k)"
echo "DBSIZE after access: $($CLI dbsize)"

echo "== B. active expiry off, server blocked 1.4 s by DEBUG SLEEP right after SET; EXISTS 1.5 s after SET returned"
$CLI set k2 v EX 1
( $CLI debug sleep 1.4 >/dev/null ) &
sleep 1.5
echo "EXISTS k2: $($CLI exists k2)"
wait

echo "== C. the PR's TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis at head, active expiry off, 10 runs"
cd "$CLONE/weed/filer/redis2"
RUN_REDIS_TESTS=1 REDIS_ADDR=127.0.0.1:$PORT "$SCRATCH/redis2-head.test" \
  -test.run '^TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis$' -test.count 10 -test.v 2>&1 | grep -E '^(---|ok|FAIL|PASS)|redis did not expire' | sort | uniq -c

echo "== D. same test, 10 runs, while another client keeps the server busy (DEBUG SLEEP 0.3 in a loop)"
( for i in $(seq 80); do $CLI debug sleep 0.3 >/dev/null; done ) &
LOADER=$!
RUN_REDIS_TESTS=1 REDIS_ADDR=127.0.0.1:$PORT "$SCRATCH/redis2-head.test" \
  -test.run '^TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis$' -test.count 10 -test.v 2>&1 | grep -E '^(---|ok|FAIL|PASS)|redis did not expire' | sort | uniq -c
kill $LOADER 2>/dev/null; wait 2>/dev/null
echo "active-expire still off at the end: $($CLI config get hz | tr '\n' ' ') (DEBUG SET-ACTIVE-EXPIRE is not visible in CONFIG; it was set to 0 above and never reset)"
$CLI shutdown nosave >/dev/null 2>&1
echo done
