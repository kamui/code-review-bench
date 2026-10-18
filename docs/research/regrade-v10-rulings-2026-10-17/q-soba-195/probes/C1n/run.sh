#!/usr/bin/env bash
# Usage: run.sh SCRATCH
# Runs group C1n's scenarios (see ../harness/run.sh) with the soba binary built at the base and at the head.
set -eu
S=$1
HERE=$(cd "$(dirname "$0")" && pwd)
for c in base head; do
  bash "$HERE/../harness/drive.sh" "$S" "$c" "$S/$c" "$HERE/result-$c.txt" C1n-missing-lfs-file C1n-open-count PEq1-file
done
