#!/bin/sh
# The lock keeps one launcher however the pass is started; a second start exits 0.
set -eu
export HOME=/home/t3
export BENCH_RUNTIME_STORE=/home/t3/.t3/bench-runtime
export CLAUDE_CODE_DISABLE_FAST_MODE=1
export PATH=/home/t3/.local/bin:/home/t3/.local/share/mise/installs/bun/1.4.4/bin:/home/t3/.cache/code-review-bench/claude-2.1.317:/home/t3/.local/share/mise/installs/go/1.27.3/bin:/home/t3/.cargo/bin:/usr/local/bin:/usr/bin:/bin
cd /home/t3/.t3/worktrees/code-review-bench/t3-aecb23e8
exec /usr/bin/flock -n -E 0 /home/t3/.cache/second-pass-r4.lock \
  python3 -I docs/research/audit-v2-r4-2026-11-11/second-pass/launch.py run \
  --authorization docs/research/audit-v2-r4-2026-11-11/authorization.second-pass.v1.json
