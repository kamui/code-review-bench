#!/bin/sh
# D6: run a project's tests when the login role may not connect to the
# "postgres" database, so Django falls back to "the first PostgreSQL database"
# for CREATE DATABASE / DROP DATABASE (it prints a RuntimeWarning when it does).
#
# Usage: run_fallback.sh <django checkout> <python> <sock dir> <port>
# Needs once, as a superuser:
#   CREATE ROLE d6user LOGIN CREATEDB;
#   CREATE DATABASE d6app OWNER d6user;
#   REVOKE CONNECT ON DATABASE postgres FROM PUBLIC;
# Traceback frames are filtered out of the output; the exception lines are kept.
DJ=$1; PY=$2; export PROBE_PGHOST=$3 PROBE_PGPORT=$4 PROBE_USER=d6user PROBE_DB=d6app
HERE=$(cd "$(dirname "$0")" && pwd)
cd "$HERE/project" || exit 1
admin() { psql -h "$PROBE_PGHOST" -p "$PROBE_PGPORT" -U postgres -d postgres -Atc "$1"; }
reset() {
  for db in $(admin "SELECT datname FROM pg_database WHERE datname LIKE 'test_d6app%'"); do
    admin "DROP DATABASE \"$db\" WITH (FORCE)" >/dev/null
  done
  admin "DROP DATABASE IF EXISTS d6app WITH (FORCE)" >/dev/null
  admin "CREATE DATABASE d6app OWNER d6user" >/dev/null
}
run() {
  title=$1; shift
  reset
  export PROBE_LOG=$(mktemp)
  echo "### $title"
  env "$@" PYTHONPATH="$DJ" timeout 90 "$PY" manage.py test --noinput -v 1 $PROBE_ARGS 2>&1 \
    | grep -v -E '^(Found |System check|$| |Traceback|The above exception|During handling|""")' \
    | cut -c1-330 | sed -e 's/ in [0-9.]*s$/ in <time>/' -e "s#$DJ#<django>#" | head -40
  echo "(the run is killed after 90 seconds if it has not finished)"
  echo "--- what each test process recorded"
  sort "$PROBE_LOG" | grep -v 'rows visible'; rm -f "$PROBE_LOG"
  echo "--- afterwards, seen by a superuser"
  echo "databases left over: $(admin "SELECT string_agg(datname, ', ' ORDER BY datname) FROM pg_database WHERE datname LIKE '%d6app%'")"
  echo "tables in the ORIGINAL database d6app: $(psql -h "$PROBE_PGHOST" -p "$PROBE_PGPORT" -U postgres -d d6app -Atc "SELECT coalesce(string_agg(tablename, ', ' ORDER BY tablename), '(none)') FROM pg_tables WHERE schemaname = 'public'")"
  echo
}
PYTHONPATH="$DJ" "$PY" -c "import django; print('django', django.get_version())"
PROBE_ARGS=""
run "1. no pool option, tests in one process" PROBE_POOL=0
run "2. pool enabled, tests in one process" PROBE_POOL=1
PROBE_ARGS="--parallel 2"
run "3. no pool option, --parallel 2" PROBE_POOL=0
run "4. pool enabled, --parallel 2" PROBE_POOL=1
reset
