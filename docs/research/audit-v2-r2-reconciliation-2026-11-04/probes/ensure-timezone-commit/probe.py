"""Does anything commit the SET TIME ZONE that DatabaseWrapper.ensure_timezone()
issues on a connection the wrapper already holds, with autocommit off?

No PostgreSQL server is used. The wrapper's psycopg connection is replaced by a
recording stand-in; with the pool on, the real psycopg_pool.ConnectionPool that
Django builds is used (min_size=0, so it never connects).

Usage: <venv>/bin/python -I probe.py <django tree> <label>
"""
import sys

tree, label = sys.argv[1], sys.argv[2]
sys.path.insert(0, tree)

import django
from django.conf import settings

POOL = "--pool" in sys.argv
options = {"pool": {"min_size": 0, "max_size": 1}} if POOL else {}
settings.configure(
    USE_TZ=True,
    TIME_ZONE="UTC",
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "app",
            "OPTIONS": options,
        }
    },
)
django.setup()

from django.db import connections

events = []


class FakeCursor:
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False

    def execute(self, sql, params=None):
        events.append(f"execute {sql!r} {params!r}")


class FakeInfo:
    server_version = 160000

    def parameter_status(self, name):
        return "America/Chicago" if name == "TimeZone" else None


class FakeConnection:
    info = FakeInfo()
    autocommit = False

    def cursor(self, *a, **k):
        return FakeCursor()

    def commit(self):
        events.append("commit")

    def rollback(self):
        events.append("rollback")

    def close(self):
        events.append("connection.close")


def show(title):
    print(f"[{label}] {title}")
    for e in events:
        print(f"    {e}")
    if not events:
        print("    (no events)")
    events.clear()


wrapper = connections["default"]
# Skip the server-version query that connection setup makes.
wrapper.__dict__["pg_version"] = 160000

# 1. Connection setup, autocommit off: init_connection_state().
wrapper.connection = FakeConnection()
wrapper.autocommit = False
if POOL:
    print(f"[{label}] init_connection_state skipped with the pool (head runs the pool's configure callback instead)")
else:
    wrapper.init_connection_state()
    show("init_connection_state() with autocommit off")

# 2. ensure_timezone() on a connection the wrapper already holds, autocommit off
#    (the call django.test.signals makes when TIME_ZONE or USE_TZ changes).
if POOL:
    pool = wrapper.pool
    pool.open()
    fake = FakeConnection()
    fake._pool = pool
    wrapper.connection = fake
    print(f"[{label}] pool before ensure_timezone(): closed={pool.closed}")
wrapper.ensure_timezone()
if POOL:
    print(f"[{label}] pool after ensure_timezone(): closed={pool.closed}, "
          f"alias still registered={'default' in type(wrapper)._connection_pools}")
show("ensure_timezone() on a held connection with autocommit off")

# 3. The wrapper then hands the connection back (close()).
wrapper.close()
show("close() afterwards")
if POOL:
    for p in list(type(wrapper)._connection_pools.values()):
        p.close()
