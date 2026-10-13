#!/bin/sh
# Usage: run.sh <checkout> <python> <socket dir> <port> [extra PYTHONPATH entry]
# Runs every M1 case against one Django checkout with one Python environment and prints the results.
# <port> 54471 is the live private PostgreSQL 16 server; port 54999 has no server.
set -u
here=$(cd "$(dirname "$0")" && pwd)
checkout=$1; py=$2; sock=$3; port=$4; more=${5:-}
export PYTHONPATH="$checkout${more:+:$more}:$here"
export M1_HOST="$sock" M1_PORT="$port"
pool='{"pool": true}'

echo "### API paths, OPTIONS $pool, server port $port"
for c in cursor cursor-twice ensure_connection connect orm; do "$py" "$here/probe.py" "$c" "$pool" "$sock" "$port" 2>&1; done
echo
echo "### API paths, same settings, no server listening (port 54999)"
for c in cursor connect; do "$py" "$here/probe.py" "$c" "$pool" "$sock" 54999 2>&1; done
echo
echo "### Other values of the option"
"$py" "$here/probe.py" cursor '{"pool": {"min_size": 1}}' "$sock" "$port" 2>&1
"$py" "$here/probe.py" query '{"pool": false}' "$sock" "$port" 2>&1
"$py" "$here/probe.py" query '{"pool": {}}' "$sock" "$port" 2>&1
"$py" "$here/probe.py" query '{}' "$sock" "$port" 2>&1
"$py" "$here/probe.py" cursor '{"pool": true, "_settings": {"CONN_MAX_AGE": 60}}' "$sock" "$port" 2>&1
echo
echo "### manage.py commands (python -m django <command> --settings m1proj.settings), OPTIONS $pool"
export M1_OPTIONS="$pool" PYTHONUNBUFFERED=1
run_cmd() {
  echo "\$ python -m django $1"
  sh -c "\"$py\" -m django $2" > "$here/.out" 2>&1
  status=$?
  awk 'NF' "$here/.out" | tail -n 4 | sed 's/^/    | /'
  rm -f "$here/.out"
  echo "    exit status $status"
}
run_cmd "check" "check --settings m1proj.settings"
run_cmd "check --database default" "check --database default --settings m1proj.settings"
run_cmd "migrate" "migrate --settings m1proj.settings"
run_cmd "showmigrations" "showmigrations --settings m1proj.settings"
run_cmd "inspectdb" "inspectdb --settings m1proj.settings"
run_cmd "test m1proj --noinput" "test m1proj --noinput --settings m1proj.settings"
run_cmd "dbshell -- -Atc 'select 42'" "dbshell --settings m1proj.settings -- -Atc 'select 42'"
run_cmd "shell -c 'from django.db import connection; connection.cursor()'" "shell --settings m1proj.settings -c 'from django.db import connection; connection.cursor()'"
echo "\$ python -m django runserver --noreload, then GET /q"
"$py" "$here/runserver_probe.py" 8471 2>&1
