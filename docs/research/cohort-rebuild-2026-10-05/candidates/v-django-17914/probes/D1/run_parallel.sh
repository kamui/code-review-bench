#!/bin/sh
# Usage: run_parallel.sh <django checkout> <python> <sock dir> <port>
# Runs the small project with `manage.py test --parallel 2` in several variants.
# Traceback frames are filtered out of the output; the exception lines are kept.
DJ=$1; PY=$2; export PROBE_PGHOST=$3 PROBE_PGPORT=$4
HERE=$(cd "$(dirname "$0")" && pwd)
cd "$HERE/project" || exit 1
reset() {
  # Drop test databases left behind by a killed run (superuser, outside Django).
  for db in $(psql -h "$PROBE_PGHOST" -p "$PROBE_PGPORT" -U postgres -d postgres -Atc "SELECT datname FROM pg_database WHERE datname LIKE 'test_d1%'"); do
    psql -h "$PROBE_PGHOST" -p "$PROBE_PGPORT" -U postgres -d postgres -qAtc "DROP DATABASE \"$db\" WITH (FORCE)"
  done
}
run() {
  title=$1; shift
  reset
  export PROBE_LOG=$(mktemp)
  echo "### $title"
  env "$@" PYTHONPATH="$DJ" timeout 90 "$PY" manage.py test --parallel 2 --noinput -v 1 2>&1 \
    | grep -v -E '^(Found |System check|Creating test|Cloning test|Destroying test|$| |Traceback|The above exception|During handling|""")' \
    | cut -c1-300 | sed -e 's/ in [0-9.]*s$/ in <time>/' | head -40
  echo "(the run is killed after 90 seconds if it has not finished; a missing 'Ran N tests' line means it was killed)"
  echo "--- what each process recorded"
  sort "$PROBE_LOG" | grep -v 'rows visible'; rm -f "$PROBE_LOG"
  echo
}
PYTHONPATH="$DJ" "$PY" -c "import django; print('django', django.get_version())"
run "1. no pool option, stock runner" PROBE_POOL=0 PROBE_DB_CHECK=0
run "2. no pool option, a database-tagged system check runs a query in the parent" PROBE_POOL=0 PROBE_DB_CHECK=1
run "3. pool enabled, stock runner" PROBE_POOL=1 PROBE_DB_CHECK=0
run "4. pool enabled, a database-tagged system check runs a query in the parent" PROBE_POOL=1 PROBE_DB_CHECK=1
run "5. no pool option, two database aliases, a post_migrate handler queries the default alias" PROBE_POOL=0 PROBE_DB_CHECK=0 PROBE_TWO_DBS=1 PROBE_POST_MIGRATE=1
run "6. pool enabled, two database aliases, a post_migrate handler queries the default alias" PROBE_POOL=1 PROBE_DB_CHECK=0 PROBE_TWO_DBS=1 PROBE_POST_MIGRATE=1
run "7. pool enabled, two database aliases, no handler (control)" PROBE_POOL=1 PROBE_DB_CHECK=0 PROBE_TWO_DBS=1 PROBE_POST_MIGRATE=0
reset
