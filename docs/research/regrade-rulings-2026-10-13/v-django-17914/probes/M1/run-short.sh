#!/bin/sh
# Usage: run-short.sh <checkout> <python> <socket dir> <port> [extra PYTHONPATH entry]
# The three API paths that decide M1, with OPTIONS {"pool": true}, against the live server.
set -u
here=$(cd "$(dirname "$0")" && pwd)
checkout=$1; py=$2; sock=$3; port=$4; more=${5:-}
export PYTHONPATH="$checkout${more:+:$more}:$here"
for c in cursor connect query; do "$py" "$here/probe.py" "$c" '{"pool": true}' "$sock" "$port" 2>&1; done
