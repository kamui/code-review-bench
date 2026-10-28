#!/bin/sh
# Usage: run-matrix.sh  -- compiles every probe of C1, C1n and known against base, head and each variant.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
cd "$here"
for set in C1 C1n known; do
  mkdir -p results/matrix/$set
  for tree in trees/base trees/head variants/*; do
    name=$(basename "$tree")
    node tools/check.cjs "$tree" probes/$set/*.ts | sed -e "s#$here#<scratch>#g" -e "s#^tree .*#tree $name#" > results/matrix/$set/result-$name.txt
  done
done
