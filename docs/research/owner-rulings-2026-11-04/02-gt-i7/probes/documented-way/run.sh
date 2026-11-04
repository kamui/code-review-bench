#!/bin/sh
# Usage: run.sh <site-packages of the target's venv archive> <dir holding head/src>; see ../one-context-per-pool/environment.txt
here=$(dirname "$0")
python3 -I -W ignore "$here/documented-way.py" "$1" "$2/head/src" head 2>&1
