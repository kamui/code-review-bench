"""D7: what does the caller see when per-connection setup fails?

Cases (each run in its own process)
  bad-tz           time zone name the server rejects, no pool
  bad-tz+pool      same, with OPTIONS["pool"]
  missing-role     assume_role names a role that does not exist, no pool
  missing-role+pool
  valid-role+pool  assume_role names an existing role (reference bug GT-v1's trigger)
  ok+pool          control: pool, nothing wrong
  bad-db           NAME is a database that does not exist, no pool (not a setup
  bad-db+pool      step of Django's; included to show what the pool library does
                   with any failure while opening a connection)

The pool timeout is set to 6 seconds to keep the run short. The psycopg_pool
default is 30 seconds.

Usage: PYTHONPATH=<django checkout> python probe.py <case> <sock dir> <port>
"""
import logging
import sys
import time

import django
import psycopg
from django.conf import settings

case, host, port = sys.argv[1], sys.argv[2], sys.argv[3]
options = {}
if case.endswith("+pool"):
    options["pool"] = {"timeout": 6}
if case.startswith("missing-role"):
    options["assume_role"] = "d7_no_such_role"
if case.startswith("valid-role"):
    options["assume_role"] = "d7_role"
bad_tz = case.startswith("bad-tz")
settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "d7_no_such_db" if case.startswith("bad-db") else "postgres",
            "USER": "postgres",
            "HOST": host,
            "PORT": port,
            "OPTIONS": options,
        }
    },
    INSTALLED_APPS=[],
    USE_TZ=False,
    TIME_ZONE="Mars/Olympus_Mons" if bad_tz else "UTC",
)
django.setup()
from django.db import connection

with psycopg.connect(host=host, port=port, user="postgres", dbname="postgres", autocommit=True) as admin:
    admin.execute("DROP ROLE IF EXISTS d7_role")
    admin.execute("CREATE ROLE d7_role")


class Collect(logging.Handler):
    """Collect what the pool library logs. Without any logging configuration
    Python prints these warnings to stderr."""

    def __init__(self):
        super().__init__(logging.WARNING)
        self.messages = []

    def emit(self, record):
        self.messages.append(record.getMessage().splitlines()[0])


collected = Collect()
logging.getLogger("psycopg.pool").addHandler(collected)
logging.getLogger("psycopg.pool").propagate = False

started = time.monotonic()
try:
    with connection.cursor() as cursor:
        cursor.execute("SELECT current_user, current_setting('TimeZone')")
        outcome = f"OK {cursor.fetchone()}"
except Exception as e:
    outcome = f"{type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0]}"
    cause = e.__cause__
    if cause is not None:
        outcome += f"\n      caused by {type(cause).__module__}.{type(cause).__name__}: {str(cause).splitlines()[0]}"
elapsed = time.monotonic() - started
print(f"{case}: after {elapsed:.1f}s the first query gave\n    {outcome}")
if collected.messages:
    distinct = list(dict.fromkeys(m[:160] for m in collected.messages))
    print(f"    the pool library logged {len(collected.messages)} warnings; distinct texts (first 3):")
    for message in distinct[:3]:
        print(f"      {message}")
