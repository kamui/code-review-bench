#!/bin/sh
# Usage: run.sh <venv python> <trees dir> <label>...
# Each label is a directory under <trees dir> holding src/requests; each case runs in a fresh process.
py=$1; trees=$2; shift 2
here=$(dirname "$0")
for label in "$@"; do
  for state in good empty nonpem badpem missing unreadable directory nossl; do
    timeout 120 "$py" -I -W ignore "$here/probe.py" "$trees/$label/src" "$label" "$state" 2>&1
  done
done
