#!/bin/sh
# Usage: run.sh <site-packages of the target's venv archive> <dir holding head/src>
# head 4089f3dc65f783beaa53cc032958ab625440d0ac, made with: git -C <mirror> archive <sha> src | tar x -C <dir>/head
# probe.py is the round-2 probe (docs/research/audit-v2-r2-reconciliation-2026-11-04/probes/requests-6667-subclass-conn-ca-certs/probe.py), copied unchanged.
here=$(dirname "$0")
rm -rf "$2/fix-one-context"; mkdir -p "$2/fix-one-context"; cp -r "$2/head/src" "$2/fix-one-context/src"
python3 -I "$here/fix-one-context.py" "$2/head/src/requests/adapters.py" "$2/fix-one-context/src/requests/adapters.py"
for route in conn-ca-certs init-poolmanager; do
  python3 -I -W ignore "$here/probe.py" "$1" "$2/fix-one-context/src" "fix-one-context" "$route" 2>&1
done
