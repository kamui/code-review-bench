#!/bin/sh
# Usage: documented-way.sh <weed binary> <work dir> <redis-cli> <redis port> <redis database> <port base>
#
# The documented way to do what S1 is about, with no fault and nothing forced: a filer on the
# redis2 store, a file stored with a time-to-live through the HTTP API (wiki Filer-Server-API:
# "POST /path/to/file?ttl=1d"), listed (wiki Directories-and-Files: curl -H "Accept:
# application/json" "http://localhost:8888/path/to"), left to expire, listed again, stored again
# and listed again. It prints what the HTTP listing returns and what Redis holds at each step.
set -eu
weed=$1; work=$2; cli=$3; rport=$4; rdb=$5; base=$6
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
  -filer -filer.port="$fport" -master.volumeSizeLimitMB=64 > "$work/server.log" 2>&1) &
pid=$!
trap 'kill $pid 2>/dev/null || true; wait $pid 2>/dev/null || true' EXIT

r() { "$cli" -p "$rport" -n "$rdb" "$@"; }
list() { curl -s -H "Accept: application/json" "http://127.0.0.1:$fport/probe/dir/" | python3 -I -c 'import json,sys
d=json.load(sys.stdin); print("HTTP listing of /probe/dir/:", [e["FullPath"] for e in (d.get("Entries") or [])])'; }
redis_state() {
  printf 'redis: value key /probe/dir/f.txt exists=%s ttl=%s; index members of /probe/dir=' "$(r exists /probe/dir/f.txt)" "$(r ttl /probe/dir/f.txt)"
  r eval 'return redis.call("ZRANGE", ARGV[1] .. string.char(0), 0, -1)' 0 /probe/dir | tr '\n' ' '; echo
}

i=0
until curl -s -o /dev/null "http://127.0.0.1:$fport/" ; do i=$((i + 1)); [ $i -gt 120 ] && { echo "filer did not start"; tail -20 "$work/server.log"; exit 1; }; sleep 1; done
sleep 8
echo "store: redis2 filer store, $(grep -c 'redis2' "$work/server.log") log lines name redis2; first: $(grep -m1 -o 'configure filer for redis2\|Configure filer for redis2\|redis2' "$work/server.log" | head -1)"

echo "payload" > "$work/f.txt"
echo "== 1. store /probe/dir/f.txt with ttl=1m"
i=0
until curl -s -f -F "file=@$work/f.txt" "http://127.0.0.1:$fport/probe/dir/f.txt?ttl=1m" > "$work/upload1.json"; do i=$((i + 1)); [ $i -gt 30 ] && { echo "upload failed"; cat "$work/upload1.json"; exit 1; }; sleep 2; done
cat "$work/upload1.json"; echo
list; redis_state

echo "== 2. wait 70 seconds, past the time-to-live"
sleep 70
redis_state

echo "== 3. list the directory (the listing that meets a name whose value is gone)"
list; redis_state

echo "== 4. store the same path again, no time-to-live"
curl -s -f -F "file=@$work/f.txt" "http://127.0.0.1:$fport/probe/dir/f.txt"; echo
list; redis_state

echo "== 5. list once more"
list; redis_state
echo "== server log lines about the listing"
grep -c 'no entry is found in filer store' "$work/server.log" || true
