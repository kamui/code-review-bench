#!/usr/bin/env bash
# Usage: measure-memory.sh <scratch directory> > results/memory.txt
# memory.ts for one streamed 100 MB upload, once through c.req.parseBody() and once through the exported parseBody()
# on a platform Request, three fresh processes per variant, variants in turn round after round.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
scratch="$1"
variants="base head fix-reuse fix-stream-both fix-reuse-and-stream fix-native-lower fix-release guard-only"
echo "##### memory.ts; node $("${NODE_BIN:-node}" --version), bun $(bun --version); start $(date -u +%Y-%m-%dT%H:%M:%SZ); load $(cut -d' ' -f1-3 /proc/loadavg)"
for runtime in node bun; do
  for path in hono plain; do
    for round in 1 2 3; do
      for v in $variants; do
        echo -n "$runtime $path round=$round $v: "
        LABEL="$v" PATH_KIND="$path" FILE_MB=100 "$here/run.sh" "$here/memory.ts" "$scratch/$v" "$runtime"
      done
    done
  done
done
echo "##### end $(date -u +%Y-%m-%dT%H:%M:%SZ); load $(cut -d' ' -f1-3 /proc/loadavg)"
