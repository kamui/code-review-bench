#!/bin/sh
# Usage: make-variant.sh <GROUP> <name> <patch>...
# Copies the head tree to variants/<GROUP>/<name> and applies each patch (patch -p1).
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
group=$1; name=$2; shift 2
dest="$here/variants/$group/$name"
mkdir -p "$here/variants/$group"
if [ -d "$dest" ]; then echo "exists: $dest" >&2; exit 1; fi
cp -r "$here/trees/head" "$dest"
for p in "$@"; do (cd "$dest" && patch -p1 --quiet --no-backup-if-mismatch < "$p"); done
echo "$group/$name: $(cd "$dest" && diff -r "$here/trees/head/packages" packages | grep -c '^[<>]') changed lines"
