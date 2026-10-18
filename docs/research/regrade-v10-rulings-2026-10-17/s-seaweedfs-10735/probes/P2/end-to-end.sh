#!/bin/sh
# Usage: end-to-end.sh <weed binary> <empty work dir> <redis-cli> <redis port> <redis database> <port base> <race rounds>
#
# The documented ways, with a real "weed server -filer" on the redis2 store and nothing forced:
#   1. P2: store a file over HTTP (POST), delete it (DELETE), store it again, list the directory.
#      Then <race rounds> rounds in which a DELETE and a POST of the same path are sent at the
#      same moment (two curl processes started together), each round followed by a read of the
#      file by path and a listing of its directory; rounds in which the file reads by path and is
#      not listed are counted. This is a real race, not a forced order.
#   2. C2n: a file stored with ttl=1m and stored again over itself: redis-cli MONITOR records which
#      commands the second store sends (SET and ZADD for a create, SET alone for an update).
#   3. C1n: /c1/sub/x.txt stored; the record of the directory /c1/sub removed with a direct DEL
#      (what an eviction or an out-of-band delete leaves); /c1 listed; /c1 deleted with
#      ?recursive=true; whether /c1/sub's list of children is still in Redis.
set -eu
weed=$1; work=$2; cli=$3; rport=$4; rdb=$5; base=$6; rounds=$7
here=$(cd "$(dirname "$0")" && pwd)
mport=$((base + 1)); vport=$((base + 2)); fport=$((base + 3))
rm -rf "$work"; mkdir -p "$work/data"
cat > "$work/filer.toml" <<EOF
[redis2]
enabled = true
address = "127.0.0.1:$rport"
database = $rdb
EOF
"$cli" -p "$rport" -n "$rdb" flushdb > /dev/null
(cd "$work" && exec "$weed" server -ip=127.0.0.1 -dir="$work/data" -master.port="$mport" -volume.port="$vport" \
  -filer -filer.port="$fport" -master.volumeSizeLimitMB=64 -volume.max=100 > "$work/server.log" 2>&1) &
pid=$!
trap 'kill $pid 2>/dev/null || true; wait $pid 2>/dev/null || true' EXIT
r() { "$cli" -p "$rport" -n "$rdb" "$@"; }
i=0
until curl -s -o /dev/null "http://127.0.0.1:$fport/" ; do i=$((i + 1)); [ $i -gt 120 ] && { echo "filer did not start"; tail -20 "$work/server.log"; exit 1; }; sleep 1; done
sleep 5
echo "payload" > "$work/f.txt"
i=0
until curl -s -f -o /dev/null -F "file=@$work/f.txt" "http://127.0.0.1:$fport/warmup/f.txt"; do i=$((i + 1)); [ $i -gt 30 ] && { echo "upload failed"; exit 1; }; sleep 2; done
echo "# weed $("$weed" version | head -1); store: $(grep -m1 -o 'configured filer store to [a-z0-9_]*' "$work/server.log")"
listed() { curl -s -H "Accept: application/json" "http://127.0.0.1:$fport$1/?limit=1000" | python3 -I -c 'import json,sys
d=json.load(sys.stdin); print(" ".join(sorted(e["FullPath"].rsplit("/",1)[1] for e in (d.get("Entries") or []))))'; }
code() { curl -s -o /dev/null -w '%{http_code}' "$@"; }

echo "== 1. P2: POST, DELETE, POST, one after the other"
echo "   POST /p2/f.txt -> $(code -F "file=@$work/f.txt" "http://127.0.0.1:$fport/p2/f.txt")"
echo "   DELETE /p2/f.txt -> $(code -X DELETE "http://127.0.0.1:$fport/p2/f.txt")"
echo "   POST /p2/f.txt -> $(code -F "file=@$work/f.txt" "http://127.0.0.1:$fport/p2/f.txt")"
echo "   GET /p2/f.txt -> $(code "http://127.0.0.1:$fport/p2/f.txt"); listing of /p2: [$(listed /p2)]"
echo "== 1b. P2: $rounds rounds of DELETE and POST of /p2race/f.txt sent together"
curl -s -o /dev/null -F "file=@$work/f.txt" "http://127.0.0.1:$fport/p2race/f.txt"
(r monitor > "$work/race-monitor.txt" 2>&1) &
racemon=$!
sleep 1
n=0; unlisted=0; gone=0; both=0; bad=""
while [ $n -lt "$rounds" ]; do
  n=$((n + 1))
  r echo "round-$n" > /dev/null
  curl -s -o /dev/null -X DELETE "http://127.0.0.1:$fport/p2race/f.txt" &
  a=$!
  curl -s -o /dev/null -F "file=@$work/f.txt" "http://127.0.0.1:$fport/p2race/f.txt" &
  b=$!
  wait $a $b
  get=$(code "http://127.0.0.1:$fport/p2race/f.txt")
  l=$(listed /p2race)
  if [ "$get" = 200 ] && [ "$l" != "f.txt" ]; then
    unlisted=$((unlisted + 1)); bad="$bad $n"
    [ $unlisted -le 3 ] && echo "   round $n: GET by path $get, listing [$l]"
    curl -s -o /dev/null -X DELETE "http://127.0.0.1:$fport/p2race/f.txt"
    curl -s -o /dev/null -F "file=@$work/f.txt" "http://127.0.0.1:$fport/p2race/f.txt"
  elif [ "$get" != 200 ]; then
    gone=$((gone + 1))
    curl -s -o /dev/null -F "file=@$work/f.txt" "http://127.0.0.1:$fport/p2race/f.txt"
  else
    both=$((both + 1))
  fi
done
r echo "round-end" > /dev/null
sleep 1
kill $racemon 2>/dev/null || true
echo "   $rounds rounds: file read by path and listed: $both; file gone (the DELETE ran last): $gone; file read by path and NOT listed: $unlisted"
for b in $bad; do
  echo "   commands Redis received for /p2race in round $b, in order (redis-cli monitor; only the DELETE's and the POST's commands, before the checks):"
  python3 -I "$here/monitor-round.py" "$work/race-monitor.txt" "$b" | sed 's/^/     /'
done

echo "== 2. C2n: a file stored with ttl=1m, then stored again over itself"
curl -s -o /dev/null -F "file=@$work/f.txt" "http://127.0.0.1:$fport/c2/f.txt?ttl=1m"
sleep 1
(r monitor > "$work/monitor.txt" 2>&1) &
mon=$!
sleep 1
echo "   second POST /c2/f.txt?ttl=1m -> $(code -F "file=@$work/f.txt" "http://127.0.0.1:$fport/c2/f.txt?ttl=1m")"
sleep 1
kill $mon 2>/dev/null || true
echo "   commands Redis received for /c2 during the second store (redis-cli monitor, SET value bytes left out):"
grep '"/c2' "$work/monitor.txt" | sed -E 's/^[0-9.]+ \[[^]]*\] //; s/("SET" "[^"]*") "[^"]*(.*)/\1 <entry bytes>\2/' | cut -c1-120 | sed 's/^/     /'

echo "== 3. C1n: a directory record lost, then the parent listed and deleted recursively"
echo "   POST /c1/sub/x.txt -> $(code -F "file=@$work/f.txt" "http://127.0.0.1:$fport/c1/sub/x.txt")"
echo "   redis DEL /c1/sub (the directory's record) -> $(r del /c1/sub)"
echo "   listing of /c1: [$(listed /c1)]"
printf '%s\n' 'ZRANGE "/c1\x00" 0 -1' > "$work/q1.txt"; printf '%s\n' 'EXISTS "/c1/sub\x00"' 'ZRANGE "/c1/sub\x00" 0 -1' > "$work/q2.txt"
echo "   redis: /c1's list holds [$(r < "$work/q1.txt" | paste -sd' ' -)]; /c1/sub's list exists and holds: $(r < "$work/q2.txt" | paste -sd' ' -)"
echo "   DELETE /c1?recursive=true -> $(code -X DELETE "http://127.0.0.1:$fport/c1?recursive=true")"
echo "   redis after the delete: /c1/sub's list exists and holds: $(r < "$work/q2.txt" | paste -sd' ' -); record of /c1/sub/x.txt exists: $(r exists /c1/sub/x.txt)"
echo "   POST /c1/sub/y.txt (re-creates /c1 and /c1/sub) -> $(code -F "file=@$work/f.txt" "http://127.0.0.1:$fport/c1/sub/y.txt")"
echo "   listing of /c1/sub: [$(listed /c1/sub)]"
