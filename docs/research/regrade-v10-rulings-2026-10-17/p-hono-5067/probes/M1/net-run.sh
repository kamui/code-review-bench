#!/usr/bin/env bash
# Usage: net-run.sh <scratch directory> <upload directory> > results/net.txt
# For each runtime, upload size and variant (base, head, fix-stream-both), starts net-server.ts in a fresh process,
# sends REQUESTS multipart uploads with curl over 127.0.0.1, and prints one line per upload: curl's total time and the
# time the handler spent in c.req.parseBody(). Variants run in turn, round after round.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
scratch="$1"; uploads="$2"; requests="${REQUESTS:-6}"
echo "##### net-server.ts; node $("${NODE_BIN:-node}" --version), bun $(bun --version), $(curl --version | head -1 | cut -d' ' -f1-2); start $(date -u +%Y-%m-%dT%H:%M:%SZ); load $(cut -d' ' -f1-3 /proc/loadavg)"
port=18787
for runtime in node bun; do
  for mb in 8 32 100; do
    for round in 1 2 3; do
      for v in base head fix-stream-both; do
        port=$((port + 1))
        mkdir -p "$scratch/$v/_probe"; cp "$here/net-server.ts" "$scratch/$v/_probe/net-server.ts"
        if [ "$runtime" = bun ]; then
          (cd "$scratch/$v/_probe" && PORT=$port exec bun run net-server.ts) > /tmp/net-server-$port.log 2>&1 &
        else
          (cd "$scratch/$v/_probe" && bun build net-server.ts --target=node --outfile=net-server.node.mjs >/dev/null && PORT=$port exec "${NODE_BIN:-node}" net-server.node.mjs) > /tmp/net-server-$port.log 2>&1 &
        fi
        pid=$!
        for _ in $(seq 100); do grep -q listening /tmp/net-server-$port.log 2>/dev/null && break; sleep 0.1; done
        for i in $(seq "$requests"); do
          out=$(curl -s -F title=hello -F "file=@$uploads/upload-${mb}mb.bin;type=application/octet-stream" -w ' totalSeconds=%{time_total}' "http://127.0.0.1:$port/upload")
          echo "runtime=$runtime variant=$v uploadMb=$mb round=$round request=$i $out"
        done
        kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true
        rm -f /tmp/net-server-$port.log
      done
    done
  done
done
echo "##### end $(date -u +%Y-%m-%dT%H:%M:%SZ); load $(cut -d' ' -f1-3 /proc/loadavg)"
