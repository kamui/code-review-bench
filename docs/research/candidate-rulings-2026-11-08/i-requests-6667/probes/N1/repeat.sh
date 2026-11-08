#!/bin/sh
# Usage: repeat.sh <venv python> <trees dir> <scratch dir> <out dir> <rounds> <times> <label>...
# Runs run.sh <times> times for each label, each time with new certificates, into <out dir>/result-<label>-<rounds>-<n>.txt.
py=$1; trees=$2; scratch=$3; out=$4; rounds=$5; times=$6; shift 6
here=$(dirname "$0")
mkdir -p "$out"
for label in "$@"; do
  n=1
  while [ "$n" -le "$times" ]; do
    mkdir -p "$scratch/repeat-$label-$n"
    "$here/run.sh" "$py" "$trees" "$scratch/repeat-$label-$n" "$rounds" "$label" > "$out/result-$label-$rounds-$n.txt"
    rm -rf "$scratch/repeat-$label-$n"
    n=$((n + 1))
  done
done
