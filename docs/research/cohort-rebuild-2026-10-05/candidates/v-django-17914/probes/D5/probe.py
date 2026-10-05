"""D5: does session state set by one borrower reach the next one?

"Request 1" runs in one thread: it changes the session role, search_path and
time zone with plain SQL (committed, autocommit), then the request ends the way
Django ends every request: close_old_connections().
"Request 2" runs in another thread with its own Django connection wrapper and
reads the session state.

These modes use CONN_MAX_AGE = 0, which pooling requires.
  nopool   no pool option
  pool     OPTIONS["pool"] = {"min_size": 1, "max_size": 1} so that the one
           server session is certain to be reused
  pool-default   OPTIONS["pool"] = True (4 sessions); 8 later requests are read
  pool-reset     like "pool", plus a user-supplied psycopg_pool `reset` callback
                 that runs DISCARD ALL when a connection is returned (the
                 workaround a deployment could configure itself). Django's time
                 zone is set to America/Chicago here to show whether the reset
                 also discards what Django configured.
For comparison with what already existed before the change:
  persistent     no pool, CONN_MAX_AGE = None (persistent connections), and all
                 requests served by the same worker thread

Usage: PYTHONPATH=<django checkout> python probe.py <mode> <sock dir> <port>
"""
import sys
import threading

import django
import psycopg
from django.conf import settings

mode, host, port = sys.argv[1], sys.argv[2], sys.argv[3]
options = {}
if mode == "pool":
    options["pool"] = {"min_size": 1, "max_size": 1, "timeout": 5}
elif mode == "pool-default":
    options["pool"] = True
elif mode == "pool-reset":

    def discard_all(conn):
        conn.execute("DISCARD ALL")

    options["pool"] = {"min_size": 1, "max_size": 1, "timeout": 5, "reset": discard_all}
settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "postgres",
            "USER": "postgres",
            "HOST": host,
            "PORT": port,
            "CONN_MAX_AGE": None if mode == "persistent" else 0,
            "OPTIONS": options,
        }
    },
    INSTALLED_APPS=[],
    USE_TZ=mode != "pool-reset",
    TIME_ZONE="America/Chicago" if mode == "pool-reset" else "UTC",
)
django.setup()
from django.db import close_old_connections, connection

with psycopg.connect(host=host, port=port, user="postgres", dbname="postgres", autocommit=True) as admin:
    admin.execute("DROP ROLE IF EXISTS d5_tenant_a")
    admin.execute("CREATE ROLE d5_tenant_a")

print(f"django {django.get_version()} mode={mode}")
READ = (
    "SELECT pg_backend_pid(), current_user, current_setting('search_path'), "
    "current_setting('TimeZone')"
)


def in_thread(function):
    box = {}

    def run():
        try:
            box["value"] = function()
        except Exception as e:
            box["value"] = f"{type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0]}"
        finally:
            close_old_connections()

    if mode == "persistent":
        run()
        return box["value"]
    thread = threading.Thread(target=run)
    thread.start()
    thread.join()
    return box["value"]


def read_state():
    with connection.cursor() as cursor:
        cursor.execute(READ)
        return cursor.fetchone()


def request_one():
    with connection.cursor() as cursor:
        cursor.execute(READ)
        before = cursor.fetchone()
        cursor.execute("SET ROLE d5_tenant_a")
        cursor.execute("SET search_path TO tenant_a, public")
        cursor.execute("SET TIME ZONE 'Asia/Tokyo'")
        cursor.execute(READ)
        return before, cursor.fetchone()


fmt = "server session {0}: current_user={1} search_path={2} TimeZone={3}"
first = in_thread(request_one)
if isinstance(first, str):
    print(f"request 1 failed: {first}")
    sys.exit(0)
print("request 1 at start:   " + fmt.format(*first[0]))
print("request 1 after SETs: " + fmt.format(*first[1]))
print("request 1 ended (close_old_connections)")
for n in range(2, 10 if mode == "pool-default" else 4):
    state = in_thread(read_state)
    print(f"request {n} sees:       " + (state if isinstance(state, str) else fmt.format(*state)))
