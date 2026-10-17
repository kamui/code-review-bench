#!/bin/sh
# usage: run.sh SCRATCH PORT ; SCRATCH holds worktrees base, head, fix-gtv1, fix-thread, fix-both, fix-gtv11, a venv and the socket directory sock
set -u
S=$1; PORT=$2; HERE=$(cd "$(dirname "$0")" && pwd)
run() { tree=$1; shift; for c in "$@"; do echo "=== $tree: $c"; PYTHONPATH=$S/$tree PYTHONDONTWRITEBYTECODE=1 timeout 120 $S/venv/bin/python $HERE/probe.py "$c" $S/sock "$PORT" 2>&1 | cut -c1-420; echo; done; }
run head static static-threads static-lifetime runtime-change runtime-lifetime direct-call role-only gtv11 > $HERE/result-head.txt
run base static runtime-change direct-call role-only gtv11 > $HERE/result-base.txt
: > $HERE/result-fixes.txt
for v in fix-gtv1 fix-thread fix-both fix-gtv11; do run $v static runtime-change direct-call gtv11 >> $HERE/result-fixes.txt; done
