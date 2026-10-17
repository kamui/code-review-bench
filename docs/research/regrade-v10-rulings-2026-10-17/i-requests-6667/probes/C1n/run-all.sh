#!/bin/sh
# Usage: run-all.sh SCRATCH OUT
# SCRATCH holds trees/<name>/src (base, head and the head-<fix> variants made from fix-*.patch),
# venv-era (Python 3.12, urllib3 2.2.1, certifi 2024.2.2) and venv-now (Python 3.13, urllib3 2.8.0).
# OUT is this probe directory.
set -eu
s=$1; o=$2; here=$(dirname "$0")
mkdir -p "$s/work" "$o/results"
[ -f "$s/work/certs/ca.pem" ] || sh "$here/make-certs.sh" "$s/work/certs"
cd "$s/work"
"$s/venv-era/bin/python" -I "$here/probe.py" "$s/trees/base/src" "$s/work" --importtime 15 > "$o/result-base.txt"
"$s/venv-era/bin/python" -I "$here/probe.py" "$s/trees/head/src" "$s/work" --importtime 15 > "$o/result-head.txt"
for v in lazy-verify-true lazy guard guard-oserror both both-verify-true; do
  "$s/venv-era/bin/python" -I "$here/probe.py" "$s/trees/head-$v/src" "$s/work" --importtime 15 > "$o/results/fix-$v.txt"
done
for t in base head; do
  "$s/venv-now/bin/python" -I "$here/probe.py" "$s/trees/$t/src" "$s/work" > "$o/results/urllib3-2.8.0-$t.txt"
done
