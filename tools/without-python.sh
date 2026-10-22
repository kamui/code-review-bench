#!/bin/sh
# Run COMMAND with a PATH that holds no Python interpreter, so a maintained workflow that still reaches for one fails.
# Usage: tools/without-python.sh COMMAND [ARGS...]
# Each PATH directory holding a python* entry is replaced by a directory of links to its other entries. A program
# that names an interpreter by absolute path is not stopped; every maintained caller finds programs on PATH.
set -eu
[ $# -gt 0 ] || { echo "usage: $0 COMMAND [ARGS...]" >&2; exit 2; }
farm=$(mktemp -d "${TMPDIR:-/tmp}/without-python.XXXXXX")
filtered=
count=0
old_ifs=$IFS
IFS=:
for directory in $PATH; do
    IFS=$old_ifs
    [ -n "$directory" ] && [ -d "$directory" ] || continue
    if [ -n "$(find -H "$directory" -mindepth 1 -maxdepth 1 -name 'python*' -print)" ]; then
        count=$((count + 1))
        mkdir "$farm/$count"
        find -H "$directory" -mindepth 1 -maxdepth 1 ! -name 'python*' -exec sh -c 'ln -s "$@" "$0"' "$farm/$count" {} +
        directory=$farm/$count
    fi
    filtered=${filtered:+$filtered:}$directory
done
IFS=$old_ifs
if PATH=$filtered command -v python3 > /dev/null 2>&1 || PATH=$filtered command -v python > /dev/null 2>&1; then
    echo "$0: a Python interpreter is still on PATH" >&2
    rm -rf "$farm"
    exit 2
fi
set +e
PATH=$filtered "$@"
status=$?
rm -rf "$farm"
exit "$status"
