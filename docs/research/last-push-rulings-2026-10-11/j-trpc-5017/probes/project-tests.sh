#!/bin/sh
# Usage: project-tests.sh <PAIR> <name=tree-dir>...
# Puts each tree's packages/server/src into the monorepo checkout of the head
# (dependencies installed from the task's cached store) and runs the project's
# own type check, `tsc --noEmit -p packages/tests/tsconfig.json`.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
pair=$1; shift
for spec in "$@"; do
  name=${spec%%=*}; tree=${spec#*=}
  git -C "$here/mono" checkout --quiet -- packages/server/src
  rm -rf "$here/mono/packages/server/src"
  cp -r "$here/$tree/packages/server/src" "$here/mono/packages/server/src"
  out="$here/results/$pair/project-tests-$name.txt"
  { echo "tree $name"; echo "command: cd packages/tests && ./node_modules/.bin/tsc --noEmit --pretty false -p tsconfig.json"; } > "$out"
  code=0
  (cd "$here/mono/packages/tests" && ./node_modules/.bin/tsc --noEmit --pretty false -p tsconfig.json) >> "$out" 2>&1 || code=$?
  echo "exit $code; error lines: $(grep -c 'error TS' "$out" || true)" >> "$out"
  echo "$name: $(tail -1 "$out")"
done
git -C "$here/mono" checkout --quiet -- packages/server/src
git -C "$here/mono" status --short
