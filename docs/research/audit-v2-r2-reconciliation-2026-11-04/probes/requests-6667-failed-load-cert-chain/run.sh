#!/bin/sh
# Usage: run.sh <site-packages of the target's venv archive> <dir holding base/src and head/src>
# base 8dd3b26bf59808de24fd654699f592abf6de581e, head 4089f3dc65f783beaa53cc032958ab625440d0ac,
# each made with: git -C <mirror> archive <sha> src | tar x -C <dir>/<base|head>
# setsid and </dev/null keep OpenSSL's passphrase prompt off any terminal.
here=$(dirname "$0")
for label in base head; do
  for case in control mismatch badkey encrypted; do
    setsid -w python3 -I "$here/probe.py" "$1" "$2/$label/src" "$label" "$case" </dev/null 2>&1
  done
done
