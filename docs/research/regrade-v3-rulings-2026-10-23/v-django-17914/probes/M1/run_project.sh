#!/bin/sh
# Usage: run_project.sh <scratch> <port> <tree> <python>
# Runs manage.py test --keepdb on project/ with the stock runner and with a custom runner.
S=$1; PORT=$2; TREE=$3; PY=$4
HERE=$(cd "$(dirname "$0")" && pwd)
cd "$HERE/project" || exit 1
for db in $(psql -h "$S/sock" -p "$PORT" -U postgres -d postgres -Atc "SELECT datname FROM pg_database WHERE datname LIKE '%m1_app%'"); do
  psql -h "$S/sock" -p "$PORT" -U postgres -d postgres -qAtc "DROP DATABASE \"$db\" WITH (FORCE)"
done
psql -h "$S/sock" -p "$PORT" -U postgres -d postgres -qAtc "CREATE DATABASE m1_app"
echo "######## tree: $TREE"
if [ "$TREE" = base ]; then pools="0"; else pools="1 0"; fi
for pool in $pools; do
  for runner in django.test.runner.DiscoverRunner t.runner.RecordingRunner; do
    for n in 1 2; do
      echo "### pool=$pool runner=$runner, run $n: manage.py test --keepdb"
      start=$(date +%s.%N)
      PROBE_POOL=$pool PROBE_RUNNER=$runner PROBE_PGHOST="$S/sock" PROBE_PGPORT="$PORT" PYTHONPATH="$S/$TREE" \
        timeout 120 "$PY" -u manage.py test --keepdb -v 1 2>&1 | grep -v -E '^(Found |System check|$)' | sed -e 's/ in [0-9.]*s$/ in <time>/'
      end=$(date +%s.%N)
      echo "(process exited; wall time $(python3 -c "print(round($end - $start, 1))") s)"
      echo "sessions left on the server after the process exited: $(psql -h "$S/sock" -p "$PORT" -U postgres -d postgres -Atc "SELECT count(*) FROM pg_stat_activity WHERE backend_type = 'client backend' AND pid <> pg_backend_pid()")"
      echo
    done
  done
done
