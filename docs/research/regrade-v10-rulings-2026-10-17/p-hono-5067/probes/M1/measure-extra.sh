#!/usr/bin/env bash
# Usage: measure-extra.sh <scratch directory> > results/time-extra.txt
# The further timing cells, each line one fresh process (time.ts), variants in turn round after round:
#   mixed  a mixed-case media type on c.req.parseBody(), streamed 100 MB upload, 3 rounds
#   plain  the exported parseBody() on a platform Request, streamed 8 and 100 MB uploads, 3 rounds
#   formdata  c.req.formData(), the platform-backed reader, 8 and 100 MB, streamed and prepared, base and head, 3 rounds
#   node24.21  base and head on node 24.21.0 (NODE_BIN2), 128 MB, streamed and prepared, 5 rounds
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
scratch="$1"
all="base head fix-reuse fix-stream-both fix-reuse-and-stream fix-native-lower fix-release guard-only"
echo "##### time.ts further cells; node $("${NODE_BIN:-node}" --version) and $("$NODE_BIN2" --version), bun $(bun --version); start $(date -u +%Y-%m-%dT%H:%M:%SZ); load $(cut -d' ' -f1-3 /proc/loadavg)"
run() { # variant runtime env...
  local v="$1" rt="$2"; shift 2
  env LABEL="$v" "$@" "$here/run.sh" "$here/time.ts" "$scratch/$v" "$rt"
}
for rt in node bun; do
  for round in 1 2 3; do
    for v in $all; do run "$v" "$rt" PATH_KIND=hono MODE=stream BODY=file FILE_MB=100 CT=mixed REQUESTS=6 WARMUP=1; done
  done
  for mb in 8 100; do
    for round in 1 2 3; do
      for v in $all; do run "$v" "$rt" PATH_KIND=plain MODE=stream BODY=file FILE_MB=$mb CT=lower REQUESTS=$([ $mb = 8 ] && echo 40 || echo 6) WARMUP=1; done
    done
    for mode in stream bytes; do
      for round in 1 2 3; do
        for v in base head; do run "$v" "$rt" PATH_KIND=hono OP=formData MODE=$mode BODY=file FILE_MB=$mb CT=lower REQUESTS=$([ $mb = 8 ] && echo 40 || echo 6) WARMUP=1; done
      done
    done
  done
done
for mode in stream bytes; do
  for round in 1 2 3 4 5; do
    for v in base head; do REBUILD=1 NODE_BIN="$NODE_BIN2" run "$v" node PATH_KIND=hono MODE=$mode BODY=file FILE_MB=128 CT=lower REQUESTS=5 WARMUP=1; done
  done
done
echo "##### end $(date -u +%Y-%m-%dT%H:%M:%SZ); load $(cut -d' ' -f1-3 /proc/loadavg)"
