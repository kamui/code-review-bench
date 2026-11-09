"""After wrapper B (thread 2) calls ensure_timezone() under a TIME_ZONE override,
which wrapper does the alias's next pool configure new sessions from, and with
which time zone?

No PostgreSQL server is used. Django builds the real psycopg_pool.ConnectionPool
(min_size=0, never opened, so it never connects). A new pooled session is
stood in for by calling the callback the pool stores as `_configure` (the call
psycopg_pool makes after it opens a session) on a recording stand-in connection.

Usage: <venv>/bin/python -I probe.py <django tree> <order>
  order "creator-first": thread 1 (wrapper A) touches the pool before B does
  order "b-first":       wrapper B touches the pool first
"""
import sys
import threading

tree, order = sys.argv[1], sys.argv[2]
sys.path.insert(0, tree)

import django
from django.conf import settings

settings.configure(
    USE_TZ=False,
    TIME_ZONE="UTC",
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "app",
            "OPTIONS": {"pool": {"min_size": 0, "max_size": 2}},
        }
    },
)
django.setup()

import django.test.signals  # noqa: F401  registers the setting_changed receivers
from django.db import connections
from django.test.utils import override_settings


class FakeCursor:
    def __init__(self, log):
        self.log = log

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False

    def execute(self, sql, params=None):
        self.log.append(f"execute {sql!r} {params!r}")


class FakeInfo:
    def parameter_status(self, name):
        return "America/Chicago" if name == "TimeZone" else None


class FakeConnection:
    info = FakeInfo()

    def __init__(self):
        self.log = []

    def cursor(self, *a, **k):
        return FakeCursor(self.log)


def new_session(pool, who):
    conn = FakeConnection()
    pool._configure(conn)
    owner = pool._configure.__self__
    print(f"[{order}] new session via {who}: configure bound to wrapper "
          f"{owner.__dict__.get('_probe_name')}; {conn.log}")


a_ready = threading.Event()
b_done = threading.Event()
a_go = threading.Event()
a_finished = threading.Event()
box = {}


def thread_a():
    a = connections["default"]
    a._probe_name = "A (thread 1)"
    pool = a.pool
    box["first_pool"] = pool
    new_session(pool, "thread 1 before the override")
    print(f"[{order}] A.timezone_name cached: {a.__dict__.get('timezone_name')!r}")
    a_ready.set()
    a_go.wait()
    pool2 = a.pool
    print(f"[{order}] thread 1 touches the pool after B's override: "
          f"same object as the first pool={pool2 is box['first_pool']}")
    print(f"[{order}] A.timezone_name still cached: {a.__dict__.get('timezone_name')!r}")
    box["a_pool"] = pool2
    a_finished.set()


t = threading.Thread(target=thread_a)
t.start()
a_ready.wait()

b = connections["default"]
b._probe_name = "B (thread 2)"
with override_settings(TIME_ZONE="Africa/Nairobi"):
    first = box["first_pool"]
    print(f"[{order}] after the override in thread 2: first pool closed={first.closed}, "
          f"alias registered={'default' in type(b)._connection_pools}")
    if order == "creator-first":
        a_go.set()
        a_finished.wait()
        pool = b.pool
        print(f"[{order}] B gets the pool thread 1 rebuilt: {pool is box['a_pool']}")
        new_session(pool, "B in thread 2 under the override")
    else:
        pool = b.pool
        new_session(pool, "B in thread 2 under the override")
        a_go.set()
        a_finished.wait()
        print(f"[{order}] thread 1 gets B's pool: {box['a_pool'] is pool}")
t.join()
for p in list(type(b)._connection_pools.values()):
    p.close()
