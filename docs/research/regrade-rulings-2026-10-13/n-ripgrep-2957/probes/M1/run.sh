#!/usr/bin/env bash
# Usage: run.sh /path/to/scratch
#
# The scratch directory holds two clones of the mirror, `base` and `head`, each
# built with `cargo build --locked` (debug), a third clone `fix` of the head, and
# `bin/` for the patched binaries. Writes result-base.txt, result-head.txt and
# everything under results/ next to this script.
set -u
here=$(cd "$(dirname "$0")" && pwd)
scratch=$1
python3 -I "$here/tab_probe.py" base "$scratch/base/target/debug/rg" --json "$here/results/base.json" > "$here/result-base.txt" 2>&1
python3 -I "$here/tab_probe.py" head "$scratch/head/target/debug/rg" --json "$here/results/head.json" > "$here/result-head.txt" 2>&1
for variant in fix-n5-only fix-wrapper-only fix-n4-only fix-both; do
  ( cd "$scratch/fix" && git checkout -q -- crates/core/flags/complete/rg.zsh && git apply "$here/patches/$variant.patch" \
      && cargo build --locked && cp target/debug/rg "$scratch/bin/rg-$variant" \
      && { zsh ci/test-complete; echo "ci/test-complete exit status: $?"; } > "$here/results/ci-test-complete-$variant.txt" 2>&1 )
  python3 -I "$here/tab_probe.py" "$variant" "$scratch/bin/rg-$variant" --json "$here/results/$variant.json" > "$here/results/result-$variant.txt" 2>&1
done
python3 -I "$here/mechanism.py" > "$here/results/mechanism.txt" 2>&1
python3 -I "$here/comparable/tab_tool.py" > "$here/comparable/result.txt" 2>&1
