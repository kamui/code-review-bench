import threading
from unittest import mock

import django
from django.conf import settings

settings.configure(USE_TZ=True, TIME_ZONE="UTC", DATABASES={})
django.setup()

from django.db.backends.postgresql.base import DatabaseWrapper
import psycopg_pool


def make(alias, **opts):
    return DatabaseWrapper(
        {
            "NAME": "x", "USER": "", "PASSWORD": "", "HOST": "", "PORT": "",
            "OPTIONS": dict(opts), "TIME_ZONE": None, "CONN_MAX_AGE": 0,
            "CONN_HEALTH_CHECKS": False, "AUTOCOMMIT": True, "ATOMIC_REQUESTS": False,
            "TEST": {},
        },
        alias,
    )

# 1. close_pool()/ensure_timezone() on a wrapper that never created a pool.
created = []
orig_init = psycopg_pool.ConnectionPool.__init__
def spy_init(self, *a, **kw):
    created.append(kw.get("open"))
    return orig_init(self, *a, **kw)
with mock.patch.object(psycopg_pool.ConnectionPool, "__init__", spy_init):
    w = make("a1", pool=True)
    print("pools registered before:", dict(DatabaseWrapper._connection_pools))
    w.ensure_timezone()
    print("ConnectionPool() constructed by ensure_timezone/close_pool:", len(created))
    print("pools registered after:", dict(DatabaseWrapper._connection_pools))
    created.clear()
    w = make("a2", pool=True)
    w.pool
    n0 = len(created)
    # Count property evaluations in close_pool.
    calls = []
    orig = DatabaseWrapper.pool.fget
    with mock.patch.object(DatabaseWrapper, "pool", property(lambda s: (calls.append(1), orig(s))[1])):
        w.close_pool()
    print("pool property evaluations inside close_pool:", len(calls))

# 2. _configure_connection with assume_role reaches the Django wrapper.
w = make("a3", pool=True, assume_role="some_role")
raw = mock.MagicMock()
raw.info.parameter_status.return_value = "UTC"
hits = []
def fake_cursor(*a, **k):
    hits.append(threading.current_thread().name)
    raise RuntimeError("wrapper.cursor() called from configure hook")
with mock.patch.object(DatabaseWrapper, "cursor", fake_cursor):
    try:
        w._configure_connection(raw)
    except RuntimeError as e:
        print("configure hook ->", e, "| hits:", hits)
    else:
        print("configure hook did not touch wrapper.cursor()")
w.close_pool()
