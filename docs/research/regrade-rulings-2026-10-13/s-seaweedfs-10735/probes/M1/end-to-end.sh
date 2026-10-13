#!/bin/sh
# Usage: end-to-end.sh <weed binary> <clone of the same commit> <work dir> <redis-cli> <redis port> <redis database> <port base>
#
# The documented way, with no fault and nothing forced: "weed server -filer -s3" on the redis2
# store. Files are stored with a time-to-live through the HTTP API (wiki Filer-Server-API:
# "POST /path/to/file?ttl=1d"), other files without one, the time-to-live passes, and each
# directory is then listed the ways people list one: the filer's gRPC ListEntries, "weed shell"
# fs.ls, the S3 gateway's ListObjectsV2 and the HTTP JSON listing. It prints what each returns
# and what Redis holds. It also covers the M2 question of how a listing reaches the store's
# logical-expiry branch: a file stored with a time-to-live and stored again before it runs out.
set -eu
weed=$1; clone=$2; work=$3; cli=$4; rport=$5; rdb=$6; base=$7
here=$(cd "$(dirname "$0")" && pwd)
mport=$((base + 1)); vport=$((base + 2)); fport=$((base + 3)); sport=$((base + 4)); fgrpc=$((fport + 10000))
rm -rf "$work"; mkdir -p "$work/data"
cat > "$work/filer.toml" <<EOF
[redis2]
enabled = true
address = "127.0.0.1:$rport"
database = $rdb
EOF
"$cli" -p "$rport" -n "$rdb" flushdb > /dev/null
(cd "$work" && exec "$weed" server -ip=127.0.0.1 -dir="$work/data" -master.port="$mport" -volume.port="$vport" \
  -filer -filer.port="$fport" -s3 -s3.port="$sport" -master.volumeSizeLimitMB=64 -volume.max=100 > "$work/server.log" 2>&1) &
pid=$!
trap 'kill $pid 2>/dev/null || true; wait $pid 2>/dev/null || true' EXIT

r() { "$cli" -p "$rport" -n "$rdb" "$@"; }
names() { r eval 'return redis.call("ZCARD", ARGV[1] .. string.char(0))' 0 "$1"; }
state() { echo "   redis: the list of $1 holds $(names "$1") names; values present: $(r --scan --pattern "$1/*" | wc -l)"; }
http_list() { curl -s -H "Accept: application/json" "http://127.0.0.1:$fport$1/?limit=$2" | python3 -I -c 'import json,sys
d=json.load(sys.stdin); e=d.get("Entries") or []
print("   HTTP GET %s/?limit=%s: %d entries, ShouldDisplayLoadMore=%s, EmptyFolder=%s" % (sys.argv[1], sys.argv[2], len(e), d.get("ShouldDisplayLoadMore"), d.get("EmptyFolder")))' "$1" "$2"; }
grpc_list() { (cd "$clone" && E2E_FILER_GRPC="127.0.0.1:$fgrpc" E2E_DIR="$1" E2E_LIMIT="$2" go test -p 4 ./weed/filer/redis2/ -run 'TestProbeM1GrpcListEntries$' -v -count=1 2>&1 | grep -o 'GRPC ListEntries.*' | sed 's/^/   /') || echo "   gRPC probe failed"; }
shell_ls() { n=$(echo "fs.ls $1/" | "$weed" shell -master="127.0.0.1:$mport" -filer="127.0.0.1:$fport" 2>/dev/null | grep -c -E '^(a|b)-[0-9]+\.txt$' || true); echo "   weed shell 'fs.ls $1/': $n file names printed"; }
s3_list() { curl -s "http://127.0.0.1:$sport/$1" > "$work/s3.xml"; python3 -I -c 'import re,sys
x=open(sys.argv[1]).read()
print("   S3 GET /%s: %d <Key> elements, KeyCount=%s, IsTruncated=%s" % (sys.argv[2], len(re.findall("<Key>", x)), "".join(re.findall("<KeyCount>(.*?)</KeyCount>", x)) or "-", "".join(re.findall("<IsTruncated>(.*?)</IsTruncated>", x)) or "-"))' "$work/s3.xml" "$1"; }
put() { curl -s -f -o /dev/null -F "file=@$work/f.txt" "http://127.0.0.1:$fport$1" || echo "upload failed: $1"; }
# seed <dir> <dead> <live>: <dead> files a-0000.txt.. stored with ttl=1m, <live> files b-0000.txt.. with none
seed() {
  python3 -I -c 'import sys
d,dead,live=sys.argv[1],int(sys.argv[2]),int(sys.argv[3])
for i in range(dead): print("%s/a-%04d.txt?ttl=1m" % (d,i))
for i in range(live): print("%s/b-%04d.txt" % (d,i))' "$1" "$2" "$3" > "$work/urls.txt"
  xargs -P 8 -I{} curl -s -f -o /dev/null -F "file=@$work/f.txt" "http://127.0.0.1:$fport{}" < "$work/urls.txt" || echo "some uploads to $1 failed"
}

i=0
until curl -s -o /dev/null "http://127.0.0.1:$fport/" ; do i=$((i + 1)); [ $i -gt 120 ] && { echo "filer did not start"; tail -20 "$work/server.log"; exit 1; }; sleep 1; done
sleep 8
echo "store: $(grep -m1 -o 'redis2' "$work/server.log" | head -1) (filer.toml enables only [redis2]); weed $("$weed" version | head -1)"
echo "payload" > "$work/f.txt"
i=0
until curl -s -f -o /dev/null -F "file=@$work/f.txt" "http://127.0.0.1:$fport/warmup/f.txt"; do i=$((i + 1)); [ $i -gt 30 ] && { echo "upload failed"; exit 1; }; sleep 2; done
curl -s -o /dev/null -X PUT "http://127.0.0.1:$sport/m1bucket"

echo "== 1. store the files (a-*.txt with ttl=1m, b-*.txt with no time-to-live)"
echo "   /m2/dir/f.txt stored with ttl=1m (it is stored again in step 2)"
put "/m2/dir/f.txt?ttl=1m"
seed /m1/grpc 1024 10
seed /m1/shell 1024 10
seed /buckets/m1bucket/logs 1002 10
seed /m1/http 1 150
seed /m1/rmdir 1025 3
for d in /m1/grpc /m1/shell /buckets/m1bucket/logs /m1/http /m1/rmdir; do state "$d"; done

echo "== 2. 40 seconds later, store /m2/dir/f.txt again with ttl=1m"
sleep 40
put "/m2/dir/f.txt?ttl=1m"
echo "   redis: value of /m2/dir/f.txt exists=$(r exists /m2/dir/f.txt) ttl=$(r ttl /m2/dir/f.txt) s; the list of /m2/dir holds $(names /m2/dir) names"

echo "== 3. 35 more seconds: every a-*.txt is past its time-to-live"
sleep 35
for d in /m1/grpc /m1/shell /buckets/m1bucket/logs /m1/http /m1/rmdir; do state "$d"; done

echo "== 4. /m2/dir/f.txt, 75 s after it was first stored and 35 s after it was stored again: list the directory while redis-cli monitor records the commands"
echo "   redis before: value of /m2/dir/f.txt exists=$(r exists /m2/dir/f.txt) ttl=$(r ttl /m2/dir/f.txt) s; the list of /m2/dir holds $(names /m2/dir) names"
"$cli" -p "$rport" monitor > "$work/monitor.txt" 2>&1 &
mon=$!
sleep 1
http_list /m2/dir 100
sleep 1
kill $mon 2>/dev/null || true
grep -E '"(get|del|zrem|zrangebylex|exists|zadd|GET|DEL|ZREM|ZRANGEBYLEX|EXISTS|ZADD)"' "$work/monitor.txt" | grep '/m2/dir' | sed -E 's/^[0-9.]+ \[[^]]*\] /   redis saw: /' | cat -v
echo "   redis after: value of /m2/dir/f.txt exists=$(r exists /m2/dir/f.txt); the list of /m2/dir holds $(names /m2/dir) names"

echo "== 5. /m1/grpc (1024 expired names sort first, 10 live files follow): one gRPC ListEntries request, three times"
for n in 1 2 3; do grpc_list /m1/grpc 0; state /m1/grpc; done

echo "== 6. /m1/shell (the same shape): weed shell fs.ls, three times"
for n in 1 2 3; do shell_ls /m1/shell; state /m1/shell; done

echo "== 7. bucket m1bucket, logs/ (1002 expired names sort first, 10 live objects follow): S3 ListObjectsV2, three times"
for n in 1 2 3; do s3_list "m1bucket?list-type=2"; state /buckets/m1bucket/logs; done

echo "== 8. /m1/http (ONE expired name sorts first, 150 live files follow): HTTP JSON listing with limit=100, twice"
for n in 1 2; do http_list /m1/http 100; state /m1/http; done

echo "== 9. /m1/rmdir (1025 expired names sort first, 3 live files follow): HTTP DELETE of the directory without recursive=true"
echo "   before: GET /m1/rmdir/b-0000.txt -> HTTP $(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$fport/m1/rmdir/b-0000.txt")"
echo "   DELETE /m1/rmdir -> HTTP $(curl -s -o "$work/delete.txt" -w '%{http_code}' -X DELETE "http://127.0.0.1:$fport/m1/rmdir") $(cat "$work/delete.txt")"
echo "   after: GET /m1/rmdir/b-0000.txt -> HTTP $(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$fport/m1/rmdir/b-0000.txt")"
state /m1/rmdir
echo "   redis: record of the directory /m1/rmdir exists=$(r exists /m1/rmdir)"

echo "== server log"
echo "   lines 'no entry is found in filer store': $(grep -c 'no entry is found in filer store' "$work/server.log" || true)"
