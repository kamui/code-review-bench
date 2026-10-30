#!/bin/sh
# Cron starts this every 10 minutes and at boot, and second-pass-v4.service starts it too; the lock keeps one launcher.
set -eu
export HOME=/home/jack
export BENCH_RUNTIME_STORE=/home/jack/.cache/regrade-v4-store
export PATH=/home/jack/.local/bin:/home/jack/.local/share/mise/installs/go/1.26.5/bin:/home/jack/.local/share/mise/installs/node/24/bin:/home/jack/.local/share/mise/installs/python/3/bin:/home/jack/.cargo/bin:/home/jack/.bun/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
cd /home/jack/.cache/regrade-v4
exec /usr/bin/flock -n -E 0 /home/jack/.cache/second-pass-v4.lock \
  python3 -I docs/research/rule-v4-regrade-2026-10-25/second-pass/launch.py run \
  --authorization docs/research/rule-v4-regrade-2026-10-25/authorization.second-pass.v1.json \
  --not-before 2026-10-31T09:10:00Z
