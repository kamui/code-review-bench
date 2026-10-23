#!/bin/sh
# Usage: run.sh <scratch> <port> <tree> <python>   (writes to stdout)
# <scratch>/<tree> is a checkout of Django; <scratch>/sock holds the server's socket.
S=$1; PORT=$2; TREE=$3; PY=$4
HERE=$(cd "$(dirname "$0")" && pwd)
one() {
  pool=$1; user=$2; case=$3
  PROBE_POOL=$pool PROBE_USER=$user PYTHONPATH="$S/$TREE" timeout 120 "$PY" -u "$HERE/probe.py" "$case" "$S/sock" "$PORT" 2>&1 \
    | grep -v -E '^(Traceback|  |The above exception|During handling|$)' | cut -c1-220
  echo
}
echo "######## tree: $TREE"
if [ "$TREE" = base ]; then pools="0"; else pools="1 0"; fi
for pool in $pools; do
  for case in keepdb destroy keepdb-close-all keepdb-then-fresh-run keepdb-then-keepdb-run gtv2-early gtv2-worker; do
    one $pool postgres $case
  done
  one $pool m1_user gtv2-fallback
done
if [ "$TREE" = base ]; then one 1 postgres keepdb; fi
