#!/bin/sh
# usage: run_parallel.sh DJANGO_CHECKOUT PYTHON SOCKET_DIR PORT PSQL
# Runs probes/C1/project with `manage.py test --parallel 2` (Python 3.10 on Linux: the fork start method).
DJ=$1; PY=$2; export PROBE_PGHOST=$3 PROBE_PGPORT=$4; PSQL=$5
HERE=$(cd "$(dirname "$0")" && pwd)
cd "$HERE/project" || exit 1
reset() {
  for db in $($PSQL -h "$PROBE_PGHOST" -p "$PROBE_PGPORT" -U probe -d postgres -Atc "SELECT datname FROM pg_database WHERE datname LIKE 'test_c1app%'"); do
    $PSQL -h "$PROBE_PGHOST" -p "$PROBE_PGPORT" -U probe -d postgres -qAtc "DROP DATABASE \"$db\" WITH (FORCE)"
  done
}
run() {
  title=$1; shift
  reset
  export PROBE_LOG=$(mktemp)
  echo "### $title"
  env "$@" PYTHONPATH="$DJ" PYTHONDONTWRITEBYTECODE=1 timeout 90 "$PY" manage.py test --parallel 2 --noinput -v 1 $EXTRA 2>&1 \
    | grep -v -E '^(Found |System check|Creating test|Cloning test|Destroying test|$| |Traceback|The above exception|During handling|-- |\(|""")' \
    | cut -c1-300 | sed -e 's/ in [0-9.]*s$/ in <time>/' | head -30
  echo "(killed after 90 seconds if not finished; no 'Ran N tests' line means it was killed)"
  echo "--- what each process recorded"
  sort "$PROBE_LOG"; rm -f "$PROBE_LOG"
  echo
}
PYTHONPATH="$DJ" "$PY" -c "import django, multiprocessing; print('django', django.get_version(), 'start method', multiprocessing.get_start_method())"
EXTRA=""
run "1. no pool option, stock runner" PROBE_POOL=0 PROBE_DB_CHECK=0
run "2. no pool option, a database-tagged system check queries in the parent" PROBE_POOL=0 PROBE_DB_CHECK=1
if [ "${BASE_ONLY:-0}" != 1 ]; then
run "3. pool on, stock runner" PROBE_POOL=1 PROBE_DB_CHECK=0
run "4. pool on, a database-tagged system check queries in the parent" PROBE_POOL=1 PROBE_DB_CHECK=1
EXTRA="--debug-sql"
run "5. pool on, stock runner, --debug-sql" PROBE_POOL=1 PROBE_DB_CHECK=0
fi
reset
