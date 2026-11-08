#!/bin/sh
# Usage: run.sh <venv python> <trees dir> <scratch dir> <label>...
# Each label is a directory under <trees dir> holding src/requests; each runs in a fresh process with new certificates.
py=$1; trees=$2; scratch=$3; shift 3
here=$(dirname "$0")
for label in "$@"; do
  timeout 300 "$py" -I -W ignore "$here/probe.py" "$trees/$label" "$label" "$scratch/n2-$label" 2>&1 | sed -e "s#$scratch#<scratch>#g"
done
