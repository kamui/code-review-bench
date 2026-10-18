#!/usr/bin/env bash
# Usage: run.sh SCRATCH
# Runs group P3's scenarios (see ../harness/run.sh) with the soba binary built at the base and at the head.
set -eu
S=$1
HERE=$(cd "$(dirname "$0")" && pwd)
for c in base head; do
  bash "$HERE/../harness/drive.sh" "$S" "$c" "$S/$c" "$HERE/result-$c.txt" P3-gitea-url-only P3-sourcehut-url-only P3-gitea-token-file-missing P3-gitea-url-only-failure-only P3-control-nothing P3-control-github-token-file-missing P3-control-gitea-complete
done
