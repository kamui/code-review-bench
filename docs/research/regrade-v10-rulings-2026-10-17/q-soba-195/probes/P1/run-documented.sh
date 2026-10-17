#!/usr/bin/env bash
# Usage: run-documented.sh SCRATCH
# Runs the README's own GIT_BACKUP_DIR_FILE example at the two ends of the period in which the README
# showed it: 20d5285 (2025-06-07, the commit that added the example) and 2c183fa^ (the last commit
# before the 2026-02-08 rewrite removed it). Both need modules that are not in the archive, so they are
# downloaded from proxy.golang.org into a separate cache in the scratch directory.
set -eu
S=$1
HERE=$(cd "$(dirname "$0")" && pwd)
C="$S/clone"
[ -d "$S/doc-2025-06-07" ] || git -C "$C" worktree add -q "$S/doc-2025-06-07" 20d5285
[ -d "$S/doc-2026-02-07" ] || git -C "$C" worktree add -q "$S/doc-2026-02-07" 2c183fa^
for d in doc-2025-06-07 doc-2026-02-07; do
  DRIVE_GOMODCACHE="$S/modcache-old" DRIVE_GOPROXY=https://proxy.golang.org,direct \
    bash "$HERE/../harness/drive.sh" "$S" "$d" "$S/$d" "$HERE/result-$d.txt" P1-file-only P1-plain
done
