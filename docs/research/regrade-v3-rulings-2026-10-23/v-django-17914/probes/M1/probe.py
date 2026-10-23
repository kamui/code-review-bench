"""Where does an alias's queries go after Django's test-database setup and teardown?

Usage: probe.py <case> <socket dir> <port>
Environment: PROBE_POOL=1 turns on OPTIONS["pool"]; PROBE_USER picks the login role.

The application database is "m1_app"; its test database is "test_m1_app".
Each case resets both databases first, using a superuser session outside Django.
"""

import os
import sys
import warnings

import psycopg

CASE, HOST, PORT = sys.argv[1], sys.argv[2], sys.argv[3]
POOL = os.environ.get("PROBE_POOL") == "1"
USER = os.environ.get("PROBE_USER", "postgres")
APP_DB, TEST_DB = "m1_app", "test_m1_app"


def admin(sql, dbname="postgres"):
    with psycopg.connect(host=HOST, port=PORT, user="postgres", dbname=dbname, autocommit=True) as c:
        cur = c.execute(sql)
        return cur.fetchall() if cur.description else None


def sessions():
    rows = admin(
        "SELECT datname, count(*) FROM pg_stat_activity WHERE backend_type = 'client backend' "
        "AND pid <> pg_backend_pid() AND datname <> 'postgres' GROUP BY datname ORDER BY datname"
    )
    return dict(rows)


def tables_named(name):
    found = []
    for (db,) in admin("SELECT datname FROM pg_database WHERE datname LIKE '%m1_app%' ORDER BY datname"):
        if admin(f"SELECT to_regclass('public.{name}') IS NOT NULL", db)[0][0]:
            found.append(db)
    return found


def reset():
    for (db,) in admin("SELECT datname FROM pg_database WHERE datname LIKE '%m1_app%'"):
        admin(f'DROP DATABASE "{db}" WITH (FORCE)')
    admin(f'CREATE DATABASE "{APP_DB}" OWNER "{USER}"')
    admin("CREATE TABLE app_marker (v text); INSERT INTO app_marker VALUES ('application')", APP_DB)
    admin(f'ALTER TABLE app_marker OWNER TO "{USER}"', APP_DB)


reset()

import django  # noqa: E402
from django.conf import settings  # noqa: E402

options = {"pool": {"min_size": 2, "max_size": 4, "timeout": 5}} if POOL else {}
settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": APP_DB,
            "USER": USER,
            "HOST": HOST,
            "PORT": PORT,
            "OPTIONS": options,
            "TEST": {"NAME": TEST_DB},
        }
    },
    INSTALLED_APPS=["django.contrib.contenttypes"],
    USE_TZ=True,
)
django.setup()
warnings.simplefilter("always")
warnings.formatwarning = lambda m, c, *a, **k: f"[{c.__name__}: {str(m)[:100]}...]\n"

from django.db import connection, connections  # noqa: E402
from django.test.utils import setup_databases, teardown_databases  # noqa: E402


def say(label, value):
    print(f"{label:<58} {value}")


def where():
    """The database a query on the alias actually reaches, and NAME as Django reports it."""
    with connection.cursor() as cur:
        cur.execute("SELECT current_database()")
        actual = cur.fetchone()[0]
    connection.close()
    return f"current_database()={actual}  settings NAME={connection.settings_dict['NAME']}"


def pool_state():
    # Read the registry directly: the pool property would create a pool.
    wrapper = connections["default"]
    pool = getattr(type(wrapper), "_connection_pools", {}).get(wrapper.alias)
    if pool is None:
        return "no pool registered for the alias"
    return f"pool registered for the alias: dbname={pool.kwargs.get('dbname')} closed={pool.closed}"


print(f"## case={CASE} pool={'on' if POOL else 'off'} user={USER}")
import django as _dj  # noqa: E402

say("django", _dj.get_version())

if CASE in ("keepdb", "destroy"):
    keepdb = CASE == "keepdb"
    old = setup_databases(verbosity=0, interactive=False, keepdb=keepdb)
    say("during the test run", where())
    teardown_databases(old, verbosity=0, keepdb=keepdb)
    say(f"after teardown_databases(keepdb={keepdb})", pool_state())
    say("server sessions held per database after teardown", sessions())
    say("a query on the alias after teardown", where())
    with connection.cursor() as cur:
        cur.execute("CREATE TABLE IF NOT EXISTS written_after_teardown (v text)")
        cur.execute("INSERT INTO written_after_teardown VALUES ('meant for the application database')")
    connection.close()
    say("databases holding the table written after teardown", tables_named("written_after_teardown"))
    try:
        with connection.cursor() as cur:
            cur.execute("SELECT v FROM app_marker")
            say("SELECT v FROM app_marker after teardown", cur.fetchall())
    except Exception as e:
        say("SELECT v FROM app_marker after teardown", f"{type(e).__name__}: {str(e).splitlines()[0]}")
    connection.close()

elif CASE == "keepdb-close-all":
    old = setup_databases(verbosity=0, interactive=False, keepdb=True)
    say("during the test run", where())
    teardown_databases(old, verbosity=0, keepdb=True)
    connections.close_all()
    say("after teardown_databases(keepdb=True) and close_all()", pool_state())
    say("server sessions held per database", sessions())
    say("a query on the alias", where())

elif CASE == "keepdb-then-fresh-run":
    old = setup_databases(verbosity=0, interactive=False, keepdb=True)
    say("first run, during the test run", where())
    teardown_databases(old, verbosity=0, keepdb=True)
    say("server sessions held per database after first teardown", sessions())
    print("second run in the same process, keepdb=False:")
    sys.stdout.flush()
    try:
        old = setup_databases(verbosity=1, interactive=False, keepdb=False)
        say("second run, during the test run", where())
        teardown_databases(old, verbosity=0, keepdb=False)
        say("second run", "completed")
    except SystemExit as e:
        say("second run", f"SystemExit({e.code})")

elif CASE == "keepdb-then-keepdb-run":
    old = setup_databases(verbosity=0, interactive=False, keepdb=True)
    teardown_databases(old, verbosity=0, keepdb=True)
    with connection.cursor() as cur:
        cur.execute("CREATE TABLE IF NOT EXISTS written_after_teardown (v text)")
        cur.execute("INSERT INTO written_after_teardown VALUES ('meant for the application database')")
    connection.close()
    say("databases holding the table written after first teardown", tables_named("written_after_teardown"))
    old = setup_databases(verbosity=0, interactive=False, keepdb=True)
    say("second keepdb run, during the test run", where())
    with connection.cursor() as cur:
        cur.execute("SELECT to_regclass('public.written_after_teardown') IS NOT NULL")
        say("second keepdb run sees that table", cur.fetchone()[0])
    connection.close()
    teardown_databases(old, verbosity=0, keepdb=True)

elif CASE == "gtv2-early":
    # GT-v2 trigger 1: the alias is queried before setup, so its pool exists.
    say("before setup", where())
    old = setup_databases(verbosity=0, interactive=False, keepdb=False)
    say("during the test run", where())
    say("databases holding django_content_type (made by migrate)", tables_named("django_content_type"))
    teardown_databases(old, verbosity=0, keepdb=False)

elif CASE == "gtv2-worker":
    # GT-v2 trigger 2, in one process: clones are made, the parent queries the
    # alias, then worker setup switches NAME to the clone (no fork here).
    old = setup_databases(verbosity=0, interactive=False, keepdb=False, parallel=2)
    say("parent after cloning", where())
    connection.creation.setup_worker_connection(1)
    say("after setup_worker_connection(1)", where())
    connection.settings_dict["NAME"] = TEST_DB
    connection.close()
    teardown_databases(old, verbosity=0, parallel=2, keepdb=False)

elif CASE == "gtv2-fallback":
    # GT-v2 trigger 3: the login role cannot connect to "postgres", so Django's
    # fallback cursor is used; --parallel 2 asks for clones.
    try:
        old = setup_databases(verbosity=1, interactive=False, keepdb=False, parallel=2)
        say("clones", "created")
    except SystemExit as e:
        say("setup_databases", f"SystemExit({e.code})")
        sys.exit(0)
    try:
        teardown_databases(old, verbosity=0, parallel=2, keepdb=False)
        say("teardown_databases", "completed")
    except Exception as e:
        say("teardown_databases", f"{type(e).__name__}: {str(e).splitlines()[0]}")

else:
    sys.exit(f"unknown case {CASE}")
