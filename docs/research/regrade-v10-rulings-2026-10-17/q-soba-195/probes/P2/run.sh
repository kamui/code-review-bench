#!/usr/bin/env bash
# Usage: run.sh SCRATCH
# Runs group P2's scenarios (see ../harness/run.sh) with the soba binary built at the base and at the head.
set -eu
S=$1
HERE=$(cd "$(dirname "$0")" && pwd)
for c in base head; do
  bash "$HERE/../harness/drive.sh" "$S" "$c" "$S/$c" "$HERE/result-$c.txt" P2-file P2-plain
done
