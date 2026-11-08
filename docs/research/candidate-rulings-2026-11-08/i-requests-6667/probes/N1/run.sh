#!/bin/sh
# Usage: run.sh <venv python> <trees dir> <scratch dir> <rounds> <label>...
# Each label is a directory under <trees dir> holding src/requests. For each label the sequential order runs in
# one fresh process, and each threaded variant in a fresh process of its own, with new certificates each time.
# A process that does not exit 0 gets an "exit" line with its status (139 is a segmentation fault).
py=$1; trees=$2; scratch=$3; rounds=$4; shift 4
here=$(dirname "$0")
one() {
  out=$scratch/out.$$
  timeout 900 "$py" -I -W ignore "$here/probe.py" "$trees/$1" "$1" "$scratch/mat-$1-$2$3" "$2" "$rounds" $3 > "$out" 2>&1
  status=$?
  sed -e "s#$scratch#<scratch>#g" -e 's#port=[0-9]*#port=<port>#g' -e 's#/[^ ]*/venv/lib/python3.13/#<venv>/lib/python3.13/#g' -e "s#$here/##g" "$out"
  rm -f "$out"
  [ "$status" -eq 0 ] || echo "  exit $status"
}
for label in "$@"; do
  one "$label" sequential
  for kind in mismatch nokey certb none; do
    one "$label" threaded "$kind"
  done
done
