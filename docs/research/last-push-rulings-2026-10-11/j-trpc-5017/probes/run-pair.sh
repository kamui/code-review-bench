#!/bin/sh
# Usage: run-pair.sh <PAIR> <name=tree-dir>...
# Optional: SUFFIX=<text> is appended to each result file name.
# Compiles every probe of the pair against each tree and saves one result file per tree.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
pair=$1; shift
mkdir -p "$here/results/$pair"
for spec in "$@"; do
  name=${spec%%=*}; tree=${spec#*=}
  node "$here/tools/check.cjs" "$here/$tree" "$here"/probes/"$pair"/*.ts \
    | sed -e "s#$here#<scratch>#g" -e "s#^tree .*#tree $name#" > "$here/results/$pair/result-$name${SUFFIX:-}.txt"
  echo "$name: $(tail -1 "$here/results/$pair/result-$name${SUFFIX:-}.txt")"
done
